import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowUpRight,
  CircleAlert,
  Clipboard,
  ExternalLink,
  LoaderCircle,
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
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";

type ConversationWorkspaceProps = {
  conversations: Conversation[];
  messages: ConversationMessage[];
  selectedConversationId: string | undefined;
  job: AgentJob | undefined;
  hasActiveJob: boolean;
  authentication: AgentAuthentication | undefined;
  authenticationRequesting: boolean;
  loading: boolean;
  error: string | undefined;
  initialDraft:
    | {
        id: string;
        text: string;
        autoSend: boolean;
      }
    | undefined;
  onSelect(conversationId: string): void;
  onInitialDraftApplied(): void;
  onStartConversation(): void;
  onStartAuthentication(): Promise<void>;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
  onResolveAction(decision: "approve" | "decline"): Promise<void>;
};

export function ConversationWorkspace({
  conversations,
  messages,
  selectedConversationId,
  job,
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
  onResolveAction,
}: ConversationWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const composer = useRef<HTMLTextAreaElement>(null);
  const pendingMessageId = useRef<string | undefined>(undefined);
  const pendingMessageText = useRef<string | undefined>(undefined);
  const appliedInitialDraftId = useRef<string | undefined>(undefined);
  const transcriptEnd = useRef<HTMLDivElement>(null);
  const isWaiting = hasActiveJob;
  const isNewConversation = !selectedConversationId;
  const canSend = authentication?.state === "ready";

  useEffect(() => {
    if (isNewConversation) return;
    transcriptEnd.current?.scrollIntoView({
      block: "end",
      behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "auto"
        : "smooth",
    });
  }, [isNewConversation, job?.state, messages]);

  async function sendText(value: string): Promise<boolean> {
    const text = value.trim();
    if (!text) return false;
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
      setDraft((current) => (current.trim() === text ? "" : current));
    }
    return sent;
  }

  useEffect(() => {
    if (!initialDraft || appliedInitialDraftId.current === initialDraft.id) {
      return;
    }
    appliedInitialDraftId.current = initialDraft.id;
    setDraft(initialDraft.text);
    onInitialDraftApplied();
    requestAnimationFrame(() => composer.current?.focus());

    if (initialDraft.autoSend && canSend) {
      void sendText(initialDraft.text);
    }
  }, [canSend, initialDraft, onInitialDraftApplied]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await sendText(draft);
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

  async function retryLastRequest(text: string) {
    await sendText(text);
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
          aria-label={copy.conversations.identity}
        >
          <MobileAssistantHeader onStartConversation={startConversation} />
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
              job={job}
              loading={loading}
              onStartConversation={startConversation}
              onResolveAction={onResolveAction}
              onRetry={retryLastRequest}
              transcriptEnd={transcriptEnd}
            />
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

function MobileAssistantHeader({
  onStartConversation,
}: {
  onStartConversation(): void;
}) {
  return (
    <header className="assistant-mobile-identity">
      <span aria-hidden="true">
        <Sparkles />
      </span>
      <div>
        <strong>{copy.conversations.identity}</strong>
        <p>{copy.conversations.mobileDescription}</p>
      </div>
      <button
        className="assistant-mobile-identity__new focus-visible-control"
        type="button"
        aria-label={copy.actions.startConversation}
        onClick={onStartConversation}
      >
        <Plus aria-hidden="true" />
      </button>
    </header>
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
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;

  return (
    <aside
      className="assistant-history"
      aria-labelledby="conversation-list-title"
      aria-busy={showingSkeleton}
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
      {conversations.length ? (
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
      ) : showingSkeleton ? (
        <LoadingConversationRows visible={skeletonVisible} />
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
  job,
  loading,
  onStartConversation,
  onResolveAction,
  onRetry,
  transcriptEnd,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "messages" | "job" | "loading"
> & {
  conversationId: string;
  onStartConversation(): void;
  onResolveAction(decision: "approve" | "decline"): Promise<void>;
  onRetry(text: string): Promise<void>;
  transcriptEnd: RefObject<HTMLDivElement | null>;
}) {
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const lastUserRequest = [...messages]
    .reverse()
    .find((message) => message.role === "user")?.content;

  return (
    <section className="assistant-thread-panel" aria-busy={showingSkeleton}>
      <header className="assistant-thread-header">
        <div>
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
        ) : showingSkeleton ? (
          <LoadingMessages visible={skeletonVisible} />
        ) : (
          <p className="assistant-thread-panel__empty">
            {copy.conversations.threadEmpty}
          </p>
        )}
        {job?.state === "waiting_approval" && job.pendingAction ? (
          <ActionApprovalPanel
            action={job.pendingAction}
            submitting={loading}
            onResolve={onResolveAction}
          />
        ) : (
          job &&
          !isTerminalJob(job.state) && <AgentProgress state={job.state} />
        )}
        {job && isFailedJob(job.state) && lastUserRequest && (
          <AgentFailure
            disabled={loading}
            request={lastUserRequest}
            onRetry={onRetry}
          />
        )}
        <div className="assistant-transcript__end" ref={transcriptEnd} />
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
          <button
            className="assistant-auth-gate__secondary focus-visible-control"
            type="button"
            onClick={() => void onStartAuthentication()}
            disabled={requesting}
          >
            {copy.actions.restartChatgptConnection}
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

function AgentFailure({
  disabled,
  request,
  onRetry,
}: {
  disabled: boolean;
  request: string;
  onRetry(text: string): Promise<void>;
}) {
  return (
    <section className="assistant-request-recovery" role="alert">
      <CircleAlert aria-hidden="true" />
      <div>
        <strong>{copy.conversations.failed}</strong>
        <p>{copy.conversations.failedDescription}</p>
      </div>
      <button
        className="assistant-request-recovery__retry focus-visible-control"
        type="button"
        disabled={disabled}
        onClick={() => void onRetry(request)}
      >
        {copy.actions.retryRequest}
      </button>
    </section>
  );
}

function ActionApprovalPanel({
  action,
  submitting,
  onResolve,
}: {
  action: NonNullable<AgentJob["pendingAction"]>;
  submitting: boolean;
  onResolve(decision: "approve" | "decline"): Promise<void>;
}) {
  const isSchedule = action.kind === "create_schedule";
  return (
    <section
      className="assistant-action-approval"
      aria-labelledby="assistant-action-approval-title"
    >
      <p className="assistant-action-approval__eyebrow">
        {copy.conversations.approvalEyebrow}
      </p>
      <h2 id="assistant-action-approval-title">
        {copy.conversations.approvalTitle}
      </h2>
      <p className="assistant-action-approval__description">
        {isSchedule
          ? formatScheduleAction(action.title, action.startsAt)
          : copy.conversations.approvalTaskDescription.replace(
              "{title}",
              action.title,
            )}
      </p>
      <div className="assistant-action-approval__actions" role="group">
        <button
          className="assistant-action-approval__approve focus-visible-control"
          type="button"
          disabled={submitting}
          onClick={() => void onResolve("approve")}
        >
          {copy.actions.approveAction}
        </button>
        <button
          className="assistant-action-approval__decline focus-visible-control"
          type="button"
          disabled={submitting}
          onClick={() => void onResolve("decline")}
        >
          {copy.actions.declineAction}
        </button>
      </div>
    </section>
  );
}

function formatScheduleAction(title: string, startsAt: string | null) {
  if (!startsAt) {
    return copy.conversations.approvalScheduleDescription.replace(
      "{title}",
      title,
    );
  }
  const time = new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(startsAt));
  return copy.conversations.approvalScheduleWithTime
    .replace("{time}", time)
    .replace("{title}", title);
}

function isTerminalJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}

function isFailedJob(state: AgentJob["state"]) {
  return ["failed", "cancelled"].includes(state);
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

function LoadingConversationRows({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="assistant-history__loading"
      label={copy.home.loadingShort}
      visible={visible}
    >
      {Array.from({ length: 3 }, (_, index) => (
        <span className="assistant-history-skeleton__row" key={index}>
          <SkeletonBlock className="skeleton--title" />
          <SkeletonBlock className="skeleton--caption" />
        </span>
      ))}
    </SkeletonGroup>
  );
}

function LoadingMessages({ visible }: { visible: boolean }) {
  return (
    <SkeletonGroup
      className="assistant-transcript__loading"
      label={copy.home.loadingShort}
      visible={visible}
    >
      <span className="assistant-message-skeleton" data-role="user">
        <span className="assistant-message-skeleton__meta">
          <SkeletonBlock className="skeleton--message-author" />
          <SkeletonBlock className="skeleton--message-time" />
        </span>
        <SkeletonBlock className="skeleton--message-line" />
      </span>
      <span className="assistant-message-skeleton" data-role="assistant">
        <span className="assistant-message-skeleton__meta">
          <SkeletonBlock className="skeleton--message-author" />
          <SkeletonBlock className="skeleton--message-time" />
        </span>
        <SkeletonBlock className="skeleton--message-line" />
        <SkeletonBlock className="skeleton--message-line-short" />
      </span>
    </SkeletonGroup>
  );
}
