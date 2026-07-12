import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowUpRight,
  CircleAlert,
  Clipboard,
  ExternalLink,
  LoaderCircle,
  MessageCircleMore,
  Plus,
  SendHorizontal,
  Sparkles,
} from "lucide-react";
import {
  type FormEvent,
  type RefObject,
  useEffect,
  useRef,
  useState,
} from "react";

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
  initialDraft: string | undefined;
  onSelect(conversationId: string): void;
  onInitialDraftApplied(): void;
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
  initialDraft,
  onSelect,
  onInitialDraftApplied,
  onStartConversation,
  onStartAuthentication,
  onSend,
}: ConversationWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const composer = useRef<HTMLTextAreaElement>(null);
  const pendingMessageId = useRef<string | undefined>(undefined);
  const pendingMessageText = useRef<string | undefined>(undefined);
  const isWaiting = hasActiveJob;
  const isNewConversation = !selectedConversationId;
  const canSend = authentication?.state === "ready";

  useEffect(() => {
    if (!initialDraft) return;
    setDraft(initialDraft);
    onInitialDraftApplied();
    requestAnimationFrame(() => composer.current?.focus());
  }, [initialDraft, onInitialDraftApplied]);

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
    setDraft("");
    onStartConversation();
    requestAnimationFrame(() => composer.current?.focus());
  }

  function chooseStarter(text: string) {
    setDraft(text);
    requestAnimationFrame(() => composer.current?.focus());
  }

  return (
    <section
      className="assistant-page assistant-page--conversation"
      data-view={isNewConversation ? "new" : "thread"}
      aria-busy={loading}
    >
      {error && (
        <p className="assistant-inline-alert" role="alert">
          {error}
        </p>
      )}

      <div className="assistant-workbench">
        <ConversationHistory
          conversations={conversations}
          loading={loading}
          selectedConversationId={selectedConversationId}
          disabled={isWaiting}
          onSelect={onSelect}
          onStartConversation={startConversation}
        />

        <section
          className="assistant-conversation"
          aria-labelledby="assistant-title"
        >
          <header className="assistant-mobile-identity">
            <span aria-hidden="true">
              <Sparkles />
            </span>
            <div>
              <strong>{copy.conversations.identity}</strong>
              <p>{copy.conversations.mobileDescription}</p>
            </div>
          </header>
          {isNewConversation ? (
            <AssistantWelcome
              authentication={authentication}
              authenticationRequesting={authenticationRequesting}
              onChooseStarter={chooseStarter}
              onStartAuthentication={onStartAuthentication}
            />
          ) : (
            <ConversationThread
              conversations={conversations}
              conversationId={selectedConversationId}
              messages={messages}
              loading={loading}
              onStartConversation={startConversation}
            />
          )}

          {isWaiting && jobState && !isTerminalJob(jobState) && (
            <AgentProgress state={jobState} />
          )}
          {jobState && isFailedJob(jobState) && (
            <p
              className="assistant-job-state assistant-job-state--error"
              role="alert"
            >
              <CircleAlert aria-hidden="true" />
              {copy.conversations.failed}
            </p>
          )}

          {canSend ? (
            <AssistantComposer
              draft={draft}
              composer={composer}
              loading={loading}
              waiting={isWaiting}
              isNewConversation={isNewConversation}
              onChange={setDraft}
              onSubmit={submit}
            />
          ) : !isNewConversation ? (
            <AssistantAuthenticationGate
              authentication={authentication}
              requesting={authenticationRequesting}
              onStartAuthentication={onStartAuthentication}
            />
          ) : null}
        </section>
      </div>
    </section>
  );
}

function ConversationHistory({
  conversations,
  loading,
  selectedConversationId,
  disabled,
  onSelect,
  onStartConversation,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "loading" | "selectedConversationId" | "onSelect"
> & {
  disabled: boolean;
  onStartConversation(): void;
}) {
  return (
    <aside
      className="assistant-history"
      aria-labelledby="conversation-list-title"
    >
      <div className="assistant-history__header">
        <div>
          <h2 id="conversation-list-title">{copy.conversations.listTitle}</h2>
        </div>
        <button
          className="assistant-history__new focus-visible-control"
          type="button"
          aria-label={copy.actions.startConversation}
          onClick={onStartConversation}
          disabled={loading || disabled}
        >
          <Plus aria-hidden="true" />
        </button>
      </div>
      {loading ? (
        <LoadingConversationRows />
      ) : conversations.length ? (
        <ul className="assistant-history__list">
          {conversations.map((conversation) => {
            const selected = conversation.id === selectedConversationId;
            return (
              <li key={conversation.id}>
                <button
                  className="assistant-history__row focus-visible-control"
                  data-selected={selected}
                  type="button"
                  onClick={() => onSelect(conversation.id)}
                  aria-current={selected ? "page" : undefined}
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
        <p className="assistant-history__empty">{copy.conversations.empty}</p>
      )}
    </aside>
  );
}

function AssistantWelcome({
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
    <section className="assistant-welcome-panel">
      <header>
        <p className="assistant-welcome-panel__identity">
          <Sparkles aria-hidden="true" />
          {copy.conversations.identity}
        </p>
        <h1 id="assistant-title">{copy.conversations.title}</h1>
        <span>{copy.conversations.description}</span>
      </header>
      {canSend ? (
        <section
          className="assistant-starters"
          aria-labelledby="assistant-starters-title"
        >
          <h2
            className="assistant-starter-list__label"
            id="assistant-starters-title"
          >
            {copy.conversations.startersLabel}
          </h2>
          <div className="assistant-starter-list">
            {copy.conversations.starters.map((starter) => (
              <button
                className="assistant-starter-list__item focus-visible-control"
                type="button"
                key={starter}
                onClick={() => onChooseStarter(starter)}
              >
                <span>{starter}</span>
                <ArrowUpRight aria-hidden="true" />
              </button>
            ))}
          </div>
        </section>
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

function ConversationThread({
  conversations,
  conversationId,
  messages,
  loading,
  onStartConversation,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "messages" | "loading"
> & {
  conversationId: string;
  onStartConversation(): void;
}) {
  return (
    <section className="assistant-thread-panel">
      <header className="assistant-thread-header">
        <div>
          <p>{copy.conversations.threadEyebrow}</p>
          <h1 id="assistant-title">
            {selectedTitle(conversations, conversationId)}
          </h1>
          <span>{copy.conversations.threadDescription}</span>
        </div>
        <button
          className="assistant-thread-header__new focus-visible-control"
          type="button"
          aria-label={copy.actions.startConversation}
          onClick={onStartConversation}
        >
          <Plus aria-hidden="true" />
          <span>{copy.conversations.newConversation}</span>
        </button>
      </header>
      <div className="assistant-transcript" aria-live="off">
        {messages.length ? (
          <ol className="assistant-message-list">
            {messages.map((message) => {
              const streaming =
                message.role === "assistant" && message.status === "streaming";
              return (
                <li
                  key={message.id}
                  className="assistant-message"
                  data-role={message.role}
                  data-streaming={streaming}
                >
                  <div className="assistant-message__meta">
                    <strong>
                      {message.role === "user"
                        ? copy.conversations.userLabel
                        : copy.navigation.assistant}
                    </strong>
                    <time dateTime={message.createdAt}>
                      {formatMessageTime(message.createdAt)}
                    </time>
                  </div>
                  <p className="assistant-message__content">
                    {message.content}
                    {streaming && (
                      <span
                        className="assistant-message__caret"
                        aria-hidden="true"
                      />
                    )}
                  </p>
                  {streaming && (
                    <span
                      className="assistant-message__streaming"
                      role="status"
                    >
                      <LoaderCircle aria-hidden="true" className="spin" />
                      {copy.conversations.streaming}
                    </span>
                  )}
                </li>
              );
            })}
          </ol>
        ) : loading ? (
          <LoadingMessages />
        ) : (
          <p className="assistant-thread-panel__empty">
            {copy.conversations.threadEmpty}
          </p>
        )}
      </div>
    </section>
  );
}

function AssistantComposer({
  draft,
  composer,
  loading,
  waiting,
  isNewConversation,
  onChange,
  onSubmit,
}: {
  draft: string;
  composer: RefObject<HTMLTextAreaElement | null>;
  loading: boolean;
  waiting: boolean;
  isNewConversation: boolean;
  onChange(value: string): void;
  onSubmit(event: FormEvent<HTMLFormElement>): void;
}) {
  return (
    <form
      className="assistant-request-field"
      data-welcome={isNewConversation}
      onSubmit={onSubmit}
    >
      <label className="assistant-request-field__label" htmlFor="agent-message">
        {copy.conversations.composerLabel}
      </label>
      <textarea
        ref={composer}
        id="agent-message"
        value={draft}
        onChange={(event) => onChange(event.target.value)}
        maxLength={24_000}
        placeholder={copy.conversations.composerPlaceholder}
        disabled={loading || waiting}
        required
        rows={isNewConversation ? 3 : 4}
      />
      <div className="assistant-request-field__footer">
        <span>{copy.conversations.composerHelp}</span>
        <button
          className="assistant-request-field__send focus-visible-control"
          type="submit"
          aria-label={copy.actions.sendRequest}
          disabled={loading || waiting || !draft.trim()}
        >
          <SendHorizontal aria-hidden="true" />
          <span>
            {waiting ? copy.actions.sendingRequest : copy.actions.sendRequest}
          </span>
        </button>
      </div>
    </form>
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
  const [browserOpenFailed, setBrowserOpenFailed] = useState(false);
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

  async function openChatgpt() {
    if (!authentication?.verificationUrl) return;
    setBrowserOpenFailed(false);
    try {
      if (isTauri()) {
        await openUrl(authentication.verificationUrl);
      } else {
        window.open(
          authentication.verificationUrl,
          "_blank",
          "noopener,noreferrer",
        );
      }
    } catch {
      setBrowserOpenFailed(true);
    }
  }

  return (
    <section
      className="assistant-auth-gate"
      aria-live="polite"
      aria-labelledby="assistant-authentication-title"
    >
      {awaitingAuthorization ? (
        <>
          <AuthenticationHeading>
            {copy.authentication.awaitingTitle}
          </AuthenticationHeading>
          <p>{copy.authentication.awaitingDescription}</p>
          {browserOpenFailed && (
            <p className="assistant-inline-alert" role="alert">
              {copy.authentication.browserOpenFailed}
            </p>
          )}
          <div className="assistant-auth-gate__code">
            <span>{copy.authentication.codeLabel}</span>
            <output>{authentication.userCode}</output>
            <button
              className="assistant-auth-gate__copy focus-visible-control"
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
            className="assistant-auth-gate__primary focus-visible-control"
            type="button"
            onClick={() => void openChatgpt()}
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
          <p className="assistant-auth-gate__status" role="status">
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
            className="assistant-auth-gate__primary focus-visible-control"
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
    <div className="assistant-auth-gate__heading">
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
    <p className="assistant-job-state" role="status">
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
    <div className="assistant-history__loading" aria-hidden="true">
      <span className="skeleton" />
      <span className="skeleton" />
      <span className="skeleton" />
    </div>
  );
}

function LoadingMessages() {
  return (
    <div className="assistant-transcript__loading" aria-hidden="true">
      <span className="skeleton" />
      <span className="skeleton" />
    </div>
  );
}
