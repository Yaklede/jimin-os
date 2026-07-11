import {
  CircleAlert,
  LoaderCircle,
  MessageSquare,
  Plus,
  SendHorizontal,
} from "lucide-react";
import { FormEvent, useRef, useState } from "react";

import {
  type AgentJob,
  type Conversation,
  type ConversationMessage,
} from "../api/agent";
import { copy } from "../copy";
import { createUuidV7 } from "../uuid";

type ConversationWorkspaceProps = {
  conversations: Conversation[];
  messages: ConversationMessage[];
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

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = draft.trim();
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
    <section className="conversation-page" aria-busy={loading}>
      <div className="page-heading conversation-page__heading">
        <div>
          <p className="page-heading__date">{copy.conversations.kicker}</p>
          <h1>{copy.conversations.title}</h1>
        </div>
        <button
          className="secondary-button focus-visible-control"
          type="button"
          onClick={startConversation}
          disabled={loading || isWaiting}
        >
          <Plus aria-hidden="true" />
          {copy.actions.startConversation}
        </button>
      </div>

      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}

      <div className="conversation-layout">
        <aside
          className="panel conversation-directory"
          aria-labelledby="conversation-list-title"
        >
          <div className="panel__header">
            <div>
              <h2 id="conversation-list-title">
                <MessageSquare aria-hidden="true" />
                {copy.conversations.listTitle}
              </h2>
              <p>{copy.conversations.listDescription}</p>
            </div>
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
            <p className="empty-state">{copy.conversations.empty}</p>
          )}
        </aside>

        <section
          className="panel conversation-thread"
          aria-labelledby="conversation-thread-title"
        >
          <div className="panel__header conversation-thread__header">
            <div>
              <h2 id="conversation-thread-title">
                {selectedConversationId
                  ? selectedTitle(conversations, selectedConversationId)
                  : copy.conversations.newConversation}
              </h2>
              <p>{copy.conversations.threadDescription}</p>
            </div>
          </div>

          <div className="message-stream" aria-live="polite">
            {selectedConversationId && messages.length ? (
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
            ) : selectedConversationId && loading ? (
              <LoadingMessages />
            ) : (
              <p className="empty-state">{copy.conversations.threadEmpty}</p>
            )}

            {isWaiting && jobState && !isTerminalJob(jobState) && (
              <p className="conversation-status" role="status">
                <LoaderCircle aria-hidden="true" className="spin" />
                {copy.conversations.processing}
              </p>
            )}
            {jobState && isFailedJob(jobState) && (
              <p
                className="conversation-status conversation-status--error"
                role="alert"
              >
                <CircleAlert aria-hidden="true" />
                {copy.conversations.failed}
              </p>
            )}
          </div>

          <form className="conversation-composer" onSubmit={submit}>
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
              rows={4}
            />
            <div className="conversation-composer__actions">
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
