import {
  CircleAlert,
  Clipboard,
  ExternalLink,
  LoaderCircle,
  Plus,
  SendHorizontal,
  Sparkles,
} from "lucide-react";
import { FormEvent, useRef, useState } from "react";

import {
  type AgentAuthentication,
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
  authentication: AgentAuthentication | undefined;
  authenticationRequesting: boolean;
  loading: boolean;
  error: string | undefined;
  onSelect(conversationId: string): void;
  onStartConversation(): void;
  onStartAuthentication(): Promise<void>;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
};

export function ConversationWorkspace({
  conversations,
  messages,
  selectedConversationId,
  jobState,
  hasActiveJob,
  authentication,
  authenticationRequesting,
  loading,
  error,
  onSelect,
  onStartConversation,
  onStartAuthentication,
  onSend,
}: ConversationWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const composer = useRef<HTMLTextAreaElement>(null);
  const pendingMessageId = useRef<string | undefined>(undefined);
  const pendingMessageText = useRef<string | undefined>(undefined);
  const isWaiting = hasActiveJob;
  const isHome = !selectedConversationId;
  const canSend = authentication?.state === "ready";

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

  function chooseStarter(text: string) {
    setDraft(text);
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
            <h2 id="conversation-list-title">{copy.conversations.listTitle}</h2>
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
            <AssistantHome
              authentication={authentication}
              authenticationRequesting={authenticationRequesting}
              onChooseStarter={chooseStarter}
              onStartAuthentication={onStartAuthentication}
            />
          ) : (
            <section className="assistant-thread">
              <header className="assistant-thread__header">
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

          {!canSend && !isHome && (
            <AssistantAuthenticationGate
              authentication={authentication}
              requesting={authenticationRequesting}
              onStartAuthentication={onStartAuthentication}
            />
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

          {canSend && (
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
          )}
        </section>
      </div>
    </section>
  );
}

function AssistantHome({
  authentication,
  authenticationRequesting,
  onChooseStarter,
  onStartAuthentication,
}: {
  authentication: AgentAuthentication | undefined;
  authenticationRequesting: boolean;
  onChooseStarter(text: string): void;
  onStartAuthentication(): Promise<void>;
}) {
  const canSend = authentication?.state === "ready";
  return (
    <section className="assistant-start">
      <header className="assistant-welcome">
        <span className="assistant-welcome__identity">
          <Sparkles aria-hidden="true" />
          {copy.conversations.identity}
        </span>
        <h1 id="assistant-title">{copy.conversations.title}</h1>
        <span>{copy.conversations.description}</span>
      </header>
      {canSend ? (
        <div
          className="assistant-starters"
          aria-label={copy.conversations.startersLabel}
        >
          {copy.conversations.starters.map((starter) => (
            <button
              className="assistant-starter focus-visible-control"
              type="button"
              key={starter}
              onClick={() => onChooseStarter(starter)}
            >
              {starter}
            </button>
          ))}
        </div>
      ) : (
        <AssistantAuthenticationGate
          authentication={authentication}
          requesting={authenticationRequesting}
          onStartAuthentication={onStartAuthentication}
        />
      )}
    </section>
  );
}

function AssistantAuthenticationGate({
  authentication,
  requesting,
  onStartAuthentication,
}: {
  authentication: AgentAuthentication | undefined;
  requesting: boolean;
  onStartAuthentication(): Promise<void>;
}) {
  const [copied, setCopied] = useState(false);
  const awaitingAuthorization =
    authentication?.state === "awaiting_authorization" &&
    authentication.verificationUrl &&
    authentication.userCode;
  const isPreparing =
    authentication === undefined || authentication?.state === "requested";
  const hasFailed = authentication?.state === "failed";

  async function copyCode() {
    if (!authentication?.userCode) return;
    try {
      await navigator.clipboard.writeText(authentication.userCode);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }

  function openChatgpt() {
    if (!authentication?.verificationUrl) return;
    window.open(
      authentication.verificationUrl,
      "_blank",
      "noopener,noreferrer",
    );
  }

  return (
    <section
      className="assistant-authentication"
      aria-live="polite"
      aria-labelledby="assistant-authentication-title"
    >
      {awaitingAuthorization ? (
        <>
          <AuthenticationHeading>
            {copy.authentication.awaitingTitle}
          </AuthenticationHeading>
          <p>{copy.authentication.awaitingDescription}</p>
          <div className="assistant-authentication__code">
            <span>{copy.authentication.codeLabel}</span>
            <output>{authentication.userCode}</output>
            <button
              className="quiet-button focus-visible-control"
              type="button"
              onClick={() => void copyCode()}
            >
              <Clipboard aria-hidden="true" />
              {copied
                ? copy.authentication.copiedCode
                : copy.actions.copyAuthenticationCode}
            </button>
          </div>
          <button
            className="primary-button focus-visible-control"
            type="button"
            onClick={openChatgpt}
          >
            <ExternalLink aria-hidden="true" />
            {copy.actions.openChatgpt}
          </button>
        </>
      ) : isPreparing || requesting ? (
        <>
          <AuthenticationHeading>
            {copy.authentication.prepareTitle}
          </AuthenticationHeading>
          <p>{copy.authentication.prepareDescription}</p>
          <p className="assistant-authentication__status" role="status">
            <LoaderCircle aria-hidden="true" className="spin" />
            {copy.authentication.preparing}
          </p>
        </>
      ) : (
        <>
          <AuthenticationHeading>
            {hasFailed
              ? copy.authentication.failedTitle
              : copy.authentication.title}
          </AuthenticationHeading>
          <p>
            {hasFailed
              ? copy.authentication.recoveryDescription
              : copy.authentication.description}
          </p>
          <button
            className="primary-button focus-visible-control"
            type="button"
            onClick={() => void onStartAuthentication()}
            disabled={requesting}
          >
            {hasFailed
              ? copy.actions.retryChatgptConnection
              : copy.actions.connectChatgpt}
          </button>
        </>
      )}
    </section>
  );
}

function AuthenticationHeading({ children }: { children: string }) {
  return (
    <div className="assistant-authentication__heading">
      <Sparkles aria-hidden="true" />
      <h2 id="assistant-authentication-title">{children}</h2>
    </div>
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
