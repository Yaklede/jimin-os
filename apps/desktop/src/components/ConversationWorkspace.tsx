import {
  CalendarDays,
  CircleAlert,
  ClipboardCheck,
  LoaderCircle,
  MessageSquare,
  Plus,
  SendHorizontal,
  Sparkles,
} from "lucide-react";
import { FormEvent, useRef, useState } from "react";

import {
  type AgentJob,
  type Conversation,
  type ConversationMessage,
} from "../api/agent";
import { type ScheduleEntry, type Task } from "../api/planning";
import { copy } from "../copy";
import { createUuidV7 } from "../uuid";

type ConversationWorkspaceProps = {
  conversations: Conversation[];
  messages: ConversationMessage[];
  schedule: ScheduleEntry[];
  tasks: Task[];
  selectedConversationId: string | undefined;
  jobState: AgentJob["state"] | undefined;
  hasActiveJob: boolean;
  loading: boolean;
  error: string | undefined;
  onSelect(conversationId: string): void;
  onStartConversation(): void;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
};

export function ConversationWorkspace({
  conversations,
  messages,
  schedule,
  tasks,
  selectedConversationId,
  jobState,
  hasActiveJob,
  loading,
  error,
  onSelect,
  onStartConversation,
  onSend,
}: ConversationWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const composer = useRef<HTMLTextAreaElement>(null);
  const pendingMessageId = useRef<string | undefined>(undefined);
  const pendingMessageText = useRef<string | undefined>(undefined);
  const isWaiting = hasActiveJob;
  const isHome = !selectedConversationId;

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = draft.trim();
    if (!text) return;
    const clientMessageId =
      pendingMessageId.current && pendingMessageText.current === text
        ? pendingMessageId.current
        : createUuidV7();
    pendingMessageId.current = clientMessageId;
    pendingMessageText.current = text;
    const sent = await onSend(text, clientMessageId);
    if (sent) {
      pendingMessageId.current = undefined;
      pendingMessageText.current = undefined;
      setDraft("");
    }
  }

  function startConversation() {
    pendingMessageId.current = undefined;
    pendingMessageText.current = undefined;
    onStartConversation();
    composer.current?.focus();
  }

  return (
    <section className="assistant-page" aria-busy={loading}>
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}

      <div className="assistant-layout">
        <aside
          className="assistant-directory"
          aria-labelledby="conversation-list-title"
        >
          <div className="assistant-directory__header">
            <div>
              <p>{copy.conversations.kicker}</p>
              <h2 id="conversation-list-title">
                {copy.conversations.listTitle}
              </h2>
            </div>
            <button
              className="quiet-icon-button focus-visible-control"
              type="button"
              aria-label={copy.actions.startConversation}
              onClick={startConversation}
              disabled={loading || isWaiting}
            >
              <Plus aria-hidden="true" />
            </button>
          </div>
          {loading ? (
            <LoadingConversationRows />
          ) : conversations.length ? (
            <ul className="conversation-list">
              {conversations.map((conversation) => {
                const selected = conversation.id === selectedConversationId;
                return (
                  <li key={conversation.id}>
                    <button
                      className="conversation-list__row focus-visible-control"
                      data-selected={selected}
                      type="button"
                      onClick={() => onSelect(conversation.id)}
                    >
                      <strong>
                        {conversation.title ?? copy.conversations.untitled}
                      </strong>
                      <span>
                        {conversation.lastMessageAt
                          ? formatConversationTime(conversation.lastMessageAt)
                          : copy.conversations.noMessages}
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          ) : (
            <p className="assistant-directory__empty">
              {copy.conversations.empty}
            </p>
          )}
        </aside>

        <section
          className="assistant-workspace"
          aria-labelledby="assistant-title"
        >
          {isHome ? (
            <AssistantHome schedule={schedule} tasks={tasks} />
          ) : (
            <section className="assistant-thread">
              <header className="assistant-thread__header">
                <p>{copy.conversations.kicker}</p>
                <h1 id="assistant-title">
                  {selectedTitle(conversations, selectedConversationId)}
                </h1>
                <span>{copy.conversations.threadDescription}</span>
              </header>
              <div className="message-stream" aria-live="polite">
                {messages.length ? (
                  <ol className="message-list">
                    {messages.map((message) => (
                      <li
                        key={message.id}
                        className="message-row"
                        data-role={message.role}
                      >
                        <div className="message-row__meta">
                          <strong>
                            {message.role === "user"
                              ? copy.conversations.userLabel
                              : copy.productName}
                          </strong>
                          <time dateTime={message.createdAt}>
                            {formatMessageTime(message.createdAt)}
                          </time>
                        </div>
                        <p>{message.content}</p>
                      </li>
                    ))}
                  </ol>
                ) : loading ? (
                  <LoadingMessages />
                ) : (
                  <p className="assistant-thread__empty">
                    {copy.conversations.threadEmpty}
                  </p>
                )}
              </div>
            </section>
          )}

          {isWaiting && jobState && !isTerminalJob(jobState) && (
            <AgentProgress state={jobState} />
          )}
          {jobState && isFailedJob(jobState) && (
            <p
              className="assistant-progress assistant-progress--error"
              role="alert"
            >
              <CircleAlert aria-hidden="true" />
              {copy.conversations.failed}
            </p>
          )}

          <form className="assistant-composer" onSubmit={submit}>
            <label htmlFor="agent-message">
              {copy.conversations.composerLabel}
            </label>
            <textarea
              ref={composer}
              id="agent-message"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              maxLength={24_000}
              placeholder={copy.conversations.composerPlaceholder}
              disabled={loading || isWaiting}
              required
              rows={isHome ? 3 : 4}
            />
            <div className="assistant-composer__actions">
              <p>{copy.conversations.composerHelp}</p>
              <button
                className="primary-button focus-visible-control"
                type="submit"
                disabled={loading || isWaiting || !draft.trim()}
              >
                <SendHorizontal aria-hidden="true" />
                {isWaiting
                  ? copy.actions.sendingRequest
                  : copy.actions.sendRequest}
              </button>
            </div>
          </form>
        </section>
      </div>
    </section>
  );
}

function AssistantHome({
  schedule,
  tasks,
}: Pick<ConversationWorkspaceProps, "schedule" | "tasks">) {
  const nextSchedule = schedule[0];
  const nextTask = tasks[0];

  return (
    <>
      <header className="assistant-welcome">
        <Sparkles aria-hidden="true" />
        <p>{copy.conversations.kicker}</p>
        <h1 id="assistant-title">{copy.conversations.title}</h1>
        <span>{copy.conversations.description}</span>
      </header>
      <section
        className="assistant-context"
        aria-labelledby="assistant-context-title"
      >
        <div className="assistant-context__header">
          <h2 id="assistant-context-title">
            {copy.conversations.contextTitle}
          </h2>
          <p>{copy.conversations.contextDescription}</p>
        </div>
        <ul>
          <li>
            <CalendarDays aria-hidden="true" />
            <div>
              <span>{copy.conversations.nextSchedule}</span>
              <strong>
                {nextSchedule?.title ?? copy.conversations.noSchedule}
              </strong>
            </div>
            {nextSchedule && (
              <time dateTime={nextSchedule.startsAt}>
                {formatScheduleTime(nextSchedule.startsAt)}
              </time>
            )}
          </li>
          <li>
            <ClipboardCheck aria-hidden="true" />
            <div>
              <span>{copy.conversations.nextTask}</span>
              <strong>{nextTask?.title ?? copy.conversations.noTask}</strong>
            </div>
          </li>
        </ul>
      </section>
    </>
  );
}

function AgentProgress({ state }: { state: AgentJob["state"] }) {
  const message =
    state === "queued" || state === "claimed"
      ? copy.conversations.preparing
      : state === "waiting_approval"
        ? copy.conversations.waitingApproval
        : copy.conversations.processing;

  return (
    <p className="assistant-progress" role="status">
      <LoaderCircle aria-hidden="true" className="spin" />
      {message}
    </p>
  );
}

function isTerminalJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}

function isFailedJob(state: AgentJob["state"]) {
  return ["failed", "cancelled", "declined"].includes(state);
}

function selectedTitle(conversations: Conversation[], conversationId: string) {
  return (
    conversations.find((conversation) => conversation.id === conversationId)
      ?.title ?? copy.conversations.untitled
  );
}

function formatConversationTime(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
  }).format(new Date(value));
}

function formatMessageTime(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function formatScheduleTime(value: string) {
  return new Intl.DateTimeFormat("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}

function LoadingConversationRows() {
  return (
    <div className="loading-rows">
      <span className="skeleton" />
      <span className="skeleton" />
      <span className="skeleton" />
    </div>
  );
}

function LoadingMessages() {
  return (
    <div className="loading-rows">
      <span className="skeleton" />
      <span className="skeleton" />
    </div>
  );
}
