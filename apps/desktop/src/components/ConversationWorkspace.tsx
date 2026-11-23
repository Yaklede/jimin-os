import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  ArrowLeft,
  ArrowUpRight,
  ArrowDown,
  CircleAlert,
  Clipboard,
  ExternalLink,
  LoaderCircle,
  Plus,
  SendHorizontal,
  Sparkles,
  Trash2,
} from "lucide-react";
import {
  type FormEvent,
  type ReactNode,
  type RefObject,
  useEffect,
  useLayoutEffect,
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
import { conversationHistoryCopy as historyCopy } from "../copy/conversationHistory";
import { homeChatEntryCopy } from "../copy/homeChatEntry";
import { ConversationDeleteDialog } from "./ConversationDeleteDialog";
import { createUuidV7 } from "../uuid";
import {
  SkeletonBlock,
  SkeletonGroup,
  useDelayedSkeleton,
} from "./ContentSkeleton";
import {
  assistantResponseAfterLatestRequest,
  assistantTurnFeedback,
} from "./conversationResponse";
import { AssistantMarkdown } from "./AssistantMarkdown";
import { LinkifiedText } from "./ExternalTextLink";
import { RepresentativeImage } from "./RepresentativeImage";
import "./assistantConversation.css";

export { assistantResponseAfterLatestRequest } from "./conversationResponse";

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
  onReturnHome?(): void;
  historyOnly?: boolean;
  historyJobs?: Readonly<Record<string, AgentJob>>;
  onReturnToHistory?(): void;
  shortcutFocus?: boolean;
  onShortcutFocusHandled?(): void;
  onInitialDraftApplied(): void;
  onStartConversation(): Promise<boolean>;
  onDeleteConversation(conversationId: string): Promise<void>;
  onStartAuthentication(): Promise<void>;
  onSend(text: string, clientMessageId: string): Promise<boolean>;
  onResolveAction(decision: "approve" | "decline"): Promise<void>;
  renderResult?(
    message: ConversationMessage,
    onContinue: () => void,
  ): ReactNode;
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
  onReturnHome,
  historyOnly = false,
  historyJobs,
  onReturnToHistory,
  shortcutFocus,
  onShortcutFocusHandled,
  onInitialDraftApplied,
  onStartConversation,
  onDeleteConversation,
  onStartAuthentication,
  onSend,
  onResolveAction,
  renderResult,
}: ConversationWorkspaceProps) {
  const [draft, setDraft] = useState("");
  const [startingNew, setStartingNew] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState<Conversation>();
  const [deleteReceipt, setDeleteReceipt] = useState(false);
  const deleteTrigger = useRef<HTMLButtonElement | null>(null);
  const workspace = useRef<HTMLElement>(null);
  const composer = useRef<HTMLTextAreaElement>(null);
  const pendingMessageId = useRef<string | undefined>(undefined);
  const pendingMessageText = useRef<string | undefined>(undefined);
  const appliedInitialDraftId = useRef<string | undefined>(undefined);
  const sendingRef = useRef(false);
  const [sending, setSending] = useState(false);
  const isWaiting = hasActiveJob || sending;
  const selectedConversation = conversations.find(
    (conversation) => conversation.id === selectedConversationId,
  );
  const isNewConversation =
    !selectedConversationId ||
    (selectedConversation?.title === null &&
      selectedConversation.lastMessageAt === null &&
      messages.length === 0 &&
      !job);
  const canSend = authentication?.state === "ready";

  useEffect(() => {
    if (
      !historyOnly &&
      canSend &&
      composer.current &&
      (shortcutFocus || window.matchMedia("(pointer: fine)").matches)
    ) {
      composer.current.focus({ preventScroll: true });
      if (shortcutFocus) onShortcutFocusHandled?.();
    }
  }, [canSend, historyOnly, shortcutFocus, onShortcutFocusHandled]);

  async function sendText(value: string): Promise<boolean> {
    const text = value.trim();
    if (!text || sendingRef.current || hasActiveJob || !canSend) return false;
    const clientMessageId =
      pendingMessageId.current && pendingMessageText.current === text
        ? pendingMessageId.current
        : createUuidV7();
    pendingMessageId.current = clientMessageId;
    pendingMessageText.current = text;
    sendingRef.current = true;
    setSending(true);
    try {
      const sent = await onSend(text, clientMessageId);
      if (sent) {
        pendingMessageId.current = undefined;
        pendingMessageText.current = undefined;
        setDraft((current) => (current.trim() === text ? "" : current));
      }
      return sent;
    } finally {
      sendingRef.current = false;
      setSending(false);
    }
  }

  useEffect(() => {
    if (!initialDraft || appliedInitialDraftId.current === initialDraft.id) {
      return;
    }
    appliedInitialDraftId.current = initialDraft.id;
    setDraft(initialDraft.text);
    onInitialDraftApplied();
    const focusFrame = requestAnimationFrame(() => composer.current?.focus());

    if (initialDraft.autoSend && canSend) {
      void sendText(initialDraft.text);
    }
    return () => cancelAnimationFrame(focusFrame);
  }, [canSend, initialDraft, onInitialDraftApplied]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    await sendText(draft);
  }

  async function startConversation() {
    if (startingNew || isWaiting) return;
    setStartingNew(true);
    pendingMessageId.current = undefined;
    pendingMessageText.current = undefined;
    setDraft("");
    composer.current?.focus({ preventScroll: true });
    try {
      await onStartConversation();
    } finally {
      setStartingNew(false);
    }
  }

  function chooseStarter(text: string) {
    setDraft(text);
    composer.current?.focus({ preventScroll: true });
  }

  function requestDelete(
    conversation: Conversation,
    trigger: HTMLButtonElement,
  ) {
    deleteTrigger.current = trigger;
    setDeleteReceipt(false);
    setDeleteTarget(conversation);
  }

  async function retryLastRequest(text: string) {
    await sendText(text);
    composer.current?.focus({ preventScroll: true });
  }
  const onBack = onReturnToHistory ?? onReturnHome;

  return (
    <section
      ref={workspace}
      className="assistant-page assistant-page--conversation"
      data-view={historyOnly ? "history" : isNewConversation ? "new" : "thread"}
      data-has-back={onBack ? true : undefined}
      aria-busy={loading}
    >
      {onBack && (
        <div className="assistant-home-navigation">
          <button
            className="assistant-home-navigation__return focus-visible-control"
            type="button"
            onClick={onBack}
          >
            <ArrowLeft aria-hidden="true" />
            {onReturnToHistory
              ? homeChatEntryCopy.returnToHistory
              : copy.conversations.returnHome}
          </button>
        </div>
      )}
      {error && (
        <p className="assistant-inline-alert" role="alert">
          {error}
        </p>
      )}
      <span className="sr-only" role="status">
        {deleteReceipt ? historyCopy.deleted : ""}
      </span>

      <div className="assistant-workbench">
        <ConversationHistory
          conversations={
            historyOnly
              ? conversations.filter((item) => item.title || item.lastMessageAt)
              : conversations
          }
          standalone={historyOnly}
          jobs={historyOnly ? historyJobs : undefined}
          loading={loading}
          selectedConversationId={selectedConversationId}
          disabled={isWaiting || startingNew}
          onSelect={onSelect}
          onDelete={requestDelete}
          onStartConversation={() => void startConversation()}
        />

        {!historyOnly && (
          <section
            className="assistant-conversation"
            aria-label={copy.conversations.identity}
          >
            <MobileAssistantHeader
              disabled={isWaiting || startingNew}
              conversations={conversations}
              selectedConversationId={selectedConversationId}
              onSelect={onSelect}
              onDelete={requestDelete}
              onStartConversation={() => void startConversation()}
            />
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
                loading={loading || startingNew}
                onStartConversation={() => void startConversation()}
                onResolveAction={onResolveAction}
                onRetry={retryLastRequest}
                renderResult={renderResult}
                onContinue={() =>
                  composer.current?.focus({ preventScroll: true })
                }
              />
            )}

            {canSend ? (
              <AssistantComposer
                draft={draft}
                composer={composer}
                loading={loading || startingNew}
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
        )}
      </div>
      {deleteTarget && (
        <ConversationDeleteDialog
          title={deleteTarget.title ?? copy.conversations.untitled}
          onDelete={() => onDeleteConversation(deleteTarget.id)}
          onClose={(deleted) => {
            setDeleteTarget(undefined);
            if (deleted) {
              setDeleteReceipt(true);
              if (selectedConversationId === deleteTarget.id) {
                pendingMessageId.current = undefined;
                pendingMessageText.current = undefined;
                setDraft("");
              }
              const fallback =
                composer.current ??
                Array.from(
                  workspace.current?.querySelectorAll<HTMLButtonElement>(
                    ".assistant-history__new, .assistant-mobile-identity__new",
                  ) ?? [],
                ).find((element) => element.getClientRects().length > 0);
              fallback?.focus({ preventScroll: true });
            } else {
              deleteTrigger.current?.focus({ preventScroll: true });
            }
            deleteTrigger.current = null;
          }}
        />
      )}
    </section>
  );
}

function MobileAssistantHeader({
  disabled,
  conversations,
  selectedConversationId,
  onSelect,
  onStartConversation,
  onDelete,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "selectedConversationId" | "onSelect"
> & {
  disabled: boolean;
  onStartConversation(): void;
  onDelete(conversation: Conversation, trigger: HTMLButtonElement): void;
}) {
  const selected = conversations.find(
    (item) => item.id === selectedConversationId,
  );
  return (
    <header className="assistant-mobile-identity">
      <span aria-hidden="true">
        <Sparkles />
      </span>
      <div>
        <strong>{copy.conversations.identity}</strong>
        {conversations.length > 0 ? (
          <select
            className="assistant-mobile-identity__history focus-visible-control"
            aria-label={copy.conversations.listTitle}
            value={selectedConversationId ?? ""}
            onChange={(event) => onSelect(event.currentTarget.value)}
          >
            <option value="" disabled>
              {copy.conversations.newConversation}
            </option>
            {conversations.map((conversation) => (
              <option key={conversation.id} value={conversation.id}>
                {conversation.title ?? copy.conversations.untitled}
              </option>
            ))}
          </select>
        ) : (
          <p>{copy.conversations.mobileDescription}</p>
        )}
      </div>
      <div className="assistant-mobile-identity__actions">
        {selected && (
          <ConversationDeleteButton
            conversation={selected}
            onDelete={onDelete}
          />
        )}
        <button
          className="assistant-mobile-identity__new focus-visible-control"
          type="button"
          aria-label={copy.conversations.newConversation}
          onClick={onStartConversation}
          disabled={disabled}
        >
          <Plus aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}

function ConversationHistory({
  conversations,
  loading,
  selectedConversationId,
  standalone,
  jobs,
  disabled,
  onSelect,
  onStartConversation,
  onDelete,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "loading" | "selectedConversationId" | "onSelect"
> & {
  disabled: boolean;
  standalone: boolean;
  jobs?: Readonly<Record<string, AgentJob>>;
  onStartConversation(): void;
  onDelete(conversation: Conversation, trigger: HTMLButtonElement): void;
}) {
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const title = useRef<HTMLHeadingElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const standaloneScroll = useRef(0);
  useLayoutEffect(() => {
    if (!standalone) return;
    if (list.current) list.current.scrollTop = standaloneScroll.current;
    title.current?.focus({ preventScroll: true });
  }, [standalone]);

  return (
    <aside
      className="assistant-history"
      aria-labelledby="conversation-list-title"
      aria-busy={showingSkeleton}
    >
      <div className="assistant-history__header">
        <div>
          {standalone ? (
            <h1 id="conversation-list-title" tabIndex={-1} ref={title}>
              {homeChatEntryCopy.history}
            </h1>
          ) : (
            <h2 id="conversation-list-title">{copy.conversations.listTitle}</h2>
          )}
        </div>
        <button
          className="assistant-history__new focus-visible-control"
          type="button"
          aria-label={copy.conversations.newConversation}
          onClick={onStartConversation}
          disabled={loading || disabled}
        >
          <Plus aria-hidden="true" />
        </button>
      </div>
      {conversations.length ? (
        <ul
          ref={list}
          className="assistant-history__list"
          onScroll={(event) => {
            if (standalone)
              standaloneScroll.current = event.currentTarget.scrollTop;
          }}
        >
          {conversations.map((conversation) => {
            const selected = conversation.id === selectedConversationId;
            const job = jobs?.[conversation.id];
            const feedback =
              job?.state !== "completed"
                ? assistantTurnFeedback(job, undefined)
                : undefined;
            return (
              <li key={conversation.id} className="assistant-history__entry">
                <button
                  className="assistant-history__row focus-visible-control"
                  data-selected={selected}
                  type="button"
                  onClick={() => {
                    if (standalone)
                      standaloneScroll.current = list.current?.scrollTop ?? 0;
                    onSelect(conversation.id);
                  }}
                  aria-current={selected ? "page" : undefined}
                >
                  <strong>
                    {conversation.title ?? copy.conversations.untitled}
                  </strong>
                  <span>
                    {feedback?.label ??
                      (conversation.lastMessageAt
                        ? formatConversationTime(conversation.lastMessageAt)
                        : copy.conversations.noMessages)}
                  </span>
                </button>
                <ConversationDeleteButton
                  conversation={conversation}
                  onDelete={onDelete}
                />
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

function ConversationDeleteButton({
  conversation,
  onDelete,
}: {
  conversation: Conversation;
  onDelete(conversation: Conversation, trigger: HTMLButtonElement): void;
}) {
  return (
    <button
      type="button"
      className="assistant-history__delete focus-visible-control"
      aria-label={historyCopy.actionFor(
        conversation.title ?? copy.conversations.untitled,
      )}
      title={historyCopy.deleteAction}
      onClick={(event) => onDelete(conversation, event.currentTarget)}
    >
      <Trash2 aria-hidden="true" />
    </button>
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
  renderResult,
  onContinue,
}: Pick<
  ConversationWorkspaceProps,
  "conversations" | "messages" | "job" | "loading"
> & {
  conversationId: string;
  onStartConversation(): void;
  onResolveAction(decision: "approve" | "decline"): Promise<void>;
  onRetry(text: string): Promise<void>;
  renderResult: ConversationWorkspaceProps["renderResult"];
  onContinue(): void;
}) {
  const skeletonVisible = useDelayedSkeleton(loading);
  const showingSkeleton = loading || skeletonVisible;
  const lastUserMessage = [...messages]
    .reverse()
    .find((message) => message.role === "user");
  const lastUserRequest = lastUserMessage?.content;
  const latestTurnAssistantMessage =
    assistantResponseAfterLatestRequest(messages);
  const transcript = useRef<HTMLDivElement>(null);
  const following = useRef(true);
  const [showLatest, setShowLatest] = useState(false);
  const latestUserId = lastUserMessage?.id;
  const feedback = assistantTurnFeedback(job, latestTurnAssistantMessage);

  function scrollToLatest() {
    const element = transcript.current;
    if (element) element.scrollTop = element.scrollHeight;
  }

  useLayoutEffect(() => {
    following.current = true;
    setShowLatest(false);
    scrollToLatest();
  }, [conversationId, latestUserId]);

  useLayoutEffect(() => {
    if (following.current) scrollToLatest();
  }, [messages, job?.state]);

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
          aria-label={copy.conversations.newConversation}
          onClick={onStartConversation}
          disabled={loading || Boolean(job && !isTerminalJob(job.state))}
        >
          <Plus aria-hidden="true" />
          <span>{copy.conversations.newConversation}</span>
        </button>
      </header>
      <div
        className="assistant-transcript"
        ref={transcript}
        aria-live="off"
        onScroll={(event) => {
          const element = event.currentTarget;
          following.current =
            element.scrollHeight - element.clientHeight - element.scrollTop <=
            48;
          setShowLatest(!following.current);
        }}
      >
        {messages.length ? (
          <ol className="assistant-message-list">
            {messages.map((message) => {
              const streaming = isAssistantMessageStreaming(message, job);
              return (
                <li
                  key={message.id}
                  className="assistant-message"
                  data-role={message.role}
                  data-streaming={streaming}
                >
                  {message.role === "assistant" && (
                    <span
                      className="assistant-message__avatar"
                      aria-hidden="true"
                    >
                      <RepresentativeImage />
                    </span>
                  )}
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
                  <div className="assistant-message__content">
                    {message.role === "assistant" ? (
                      <AssistantMarkdown
                        text={message.content}
                        streaming={streaming}
                      />
                    ) : (
                      <LinkifiedText text={message.content} />
                    )}
                  </div>
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
        ) : null}
        {latestTurnAssistantMessage?.status === "completed" &&
          renderResult?.(latestTurnAssistantMessage, onContinue)}
        {job &&
          isFailedJob(job.state) &&
          lastUserRequest &&
          !hasDisplayableAssistantResponse(latestTurnAssistantMessage) && (
            <AgentFailure
              disabled={loading}
              request={lastUserRequest}
              onRetry={onRetry}
            />
          )}
      </div>
      {feedback && (
        <p
          className="assistant-turn-feedback assistant-turn-feedback--thread"
          data-tone={feedback.tone}
          role={feedback.tone === "error" ? "alert" : "status"}
        >
          {feedback.tone === "busy" && (
            <LoaderCircle aria-hidden="true" className="spin" />
          )}
          {feedback.label}
        </p>
      )}
      {showLatest && (
        <button
          className="assistant-thread-panel__latest focus-visible-control"
          type="button"
          onClick={() => {
            following.current = true;
            setShowLatest(false);
            scrollToLatest();
            onContinue();
          }}
        >
          <ArrowDown aria-hidden="true" />
          {copy.conversations.jumpToLatest}
        </button>
      )}
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
      <label className="sr-only" htmlFor="agent-message">
        {copy.conversations.composerLabel}
      </label>
      <textarea
        ref={composer}
        id="agent-message"
        aria-describedby="agent-message-help"
        value={draft}
        onChange={(event) => onChange(event.target.value)}
        maxLength={24_000}
        placeholder={
          isNewConversation
            ? copy.conversations.composerPlaceholder
            : copy.conversations.followUpPlaceholder
        }
        onKeyDown={(event) => {
          if (
            event.key !== "Enter" ||
            event.shiftKey ||
            event.nativeEvent.isComposing ||
            event.nativeEvent.keyCode === 229 ||
            !window.matchMedia("(pointer: fine)").matches
          )
            return;
          event.preventDefault();
          if (!loading && !waiting && draft.trim())
            event.currentTarget.form?.requestSubmit();
        }}
        required
        rows={1}
      />
      <div className="assistant-request-field__footer">
        <span className="sr-only" id="agent-message-help">
          {copy.conversations.composerHelp}
        </span>
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

export function hasDisplayableAssistantResponse(
  message: ConversationMessage | undefined,
): boolean {
  return Boolean(message?.content.trim());
}

export function isAssistantMessageStreaming(
  message: ConversationMessage,
  job: AgentJob | undefined,
): boolean {
  return (
    message.role === "assistant" &&
    message.status === "streaming" &&
    (!job || !isTerminalJob(job.state))
  );
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
