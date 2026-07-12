import { Server } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  bootstrapTrustedNetworkSession,
  completeTask,
  refreshDeviceSession,
  type SessionTokens,
  type Task,
} from "./api/planning";
import { type HomeSnapshot, fetchHomeSnapshot } from "./api/home";
import {
  AgentRequestError,
  createConversation,
  fetchAgentAuthentication,
  fetchAgentJob,
  fetchConversationMessages,
  fetchConversations,
  fetchLatestConversationJob,
  queueAgentTurn,
  requestAgentAuthentication,
  type AgentAuthentication,
  type AgentJob,
  type Conversation,
  type ConversationMessage,
} from "./api/agent";
import { ConversationWorkspace } from "./components/ConversationWorkspace";
import { AssistantRail, HomeWorkspace } from "./components/HomeWorkspace";
import { OsShell, type OsDestination } from "./components/OsShell";
import { copy } from "./copy";
import {
  clearDeviceSession,
  readDeviceSession,
  readOrCreateInstallationId,
  saveDeviceSession,
} from "./device-session";
import { personalServerBaseUrl } from "./server-config";
import { createUuidV7 } from "./uuid";

type AppMode =
  "configuration" | "server-unreachable" | "loading" | "ready" | "error";
type ConversationJobs = Record<string, AgentJob>;
const ACTIVE_RESPONSE_POLL_INTERVAL_MS = 250;

export default function App() {
  const apiBaseUrl = personalServerBaseUrl ?? "";
  const [tokens, setTokens] = useState<SessionTokens | undefined>(undefined);
  const [sessionLoaded, setSessionLoaded] = useState(false);
  const [mode, setMode] = useState<AppMode>("loading");
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [destination, setDestination] = useState<OsDestination>("home");
  const [homeSnapshot, setHomeSnapshot] = useState<HomeSnapshot | undefined>();
  const [homeLoading, setHomeLoading] = useState(false);
  const [homeError, setHomeError] = useState<string | undefined>();
  const [selectedConversationId, setSelectedConversationId] = useState<
    string | undefined
  >(undefined);
  const [conversationMessages, setConversationMessages] = useState<
    ConversationMessage[]
  >([]);
  const [conversationLoading, setConversationLoading] = useState(false);
  const [conversationError, setConversationError] = useState<
    string | undefined
  >(undefined);
  const [conversationJobs, setConversationJobs] = useState<ConversationJobs>(
    {},
  );
  const [agentAuthentication, setAgentAuthentication] = useState<
    AgentAuthentication | undefined
  >(undefined);
  const [authenticationRequesting, setAuthenticationRequesting] =
    useState(false);
  const pendingConversationId = useRef<string | undefined>(undefined);
  const [message, setMessage] = useState<string | undefined>(undefined);

  const bootstrapTrustedNetworkDevice = useCallback(async () => {
    setMode("loading");
    setMessage(undefined);
    try {
      const installationId = await readOrCreateInstallationId();
      const session = await bootstrapTrustedNetworkSession(
        apiBaseUrl,
        copy.personalServer.deviceName,
        installationId,
      );
      await saveDeviceSession({ tokens: session });
      setTokens(session);
    } catch {
      setMode("server-unreachable");
      setMessage(copy.messages.serverOffline);
    }
  }, [apiBaseUrl]);

  const refreshConversations = useCallback(async () => {
    if (!tokens) return;
    setConversationLoading(true);
    setConversationError(undefined);
    try {
      setConversations(
        await fetchConversations(apiBaseUrl, tokens.accessToken),
      );
    } catch {
      setConversationError(copy.messages.conversationLoadNotice);
    } finally {
      setConversationLoading(false);
    }
  }, [apiBaseUrl, tokens]);

  const loadHomeSnapshot = useCallback(async () => {
    if (!tokens) return;
    setHomeLoading(true);
    setHomeError(undefined);
    try {
      const [from, to] = currentLocalDayRange();
      setHomeSnapshot(
        await fetchHomeSnapshot(apiBaseUrl, tokens.accessToken, from, to),
      );
    } catch {
      setHomeError(copy.messages.homeLoadNotice);
    } finally {
      setHomeLoading(false);
    }
  }, [apiBaseUrl, tokens]);

  const loadConversationMessages = useCallback(
    async (conversationId: string, background = false) => {
      if (!tokens) return;
      if (!background) {
        setConversationLoading(true);
        setConversationError(undefined);
      }
      try {
        setConversationMessages(
          await fetchConversationMessages(
            apiBaseUrl,
            tokens.accessToken,
            conversationId,
          ),
        );
      } catch (error) {
        if (!background) {
          setConversationMessages([]);
          setConversationError(
            error instanceof AgentRequestError && error.code === "notFound"
              ? copy.messages.conversationChanged
              : copy.messages.conversationLoadNotice,
          );
        }
      } finally {
        if (!background) setConversationLoading(false);
      }
    },
    [apiBaseUrl, tokens],
  );

  const refresh = useCallback(async () => {
    if (!sessionLoaded) return;
    if (!tokens) return;
    setMode("loading");
    setMessage(undefined);
    try {
      const [nextConversations, authentication] = await Promise.all([
        fetchConversations(apiBaseUrl, tokens.accessToken),
        fetchAgentAuthentication(apiBaseUrl, tokens.accessToken),
        loadHomeSnapshot(),
      ]);
      setConversations(nextConversations);
      setAgentAuthentication(authentication);
      setMode("ready");
    } catch (error) {
      if (error instanceof AgentRequestError && error.code === "unauthorized") {
        try {
          const refreshed = await refreshDeviceSession(
            apiBaseUrl,
            tokens.refreshToken,
          );
          await saveDeviceSession({ tokens: refreshed });
          setTokens(refreshed);
          return;
        } catch {
          await discardSession();
        }
        return;
      }
      setMode("error");
      setMessage(copy.messages.conversationLoadNotice);
    }
  }, [apiBaseUrl, loadHomeSnapshot, sessionLoaded, tokens]);

  async function discardSession() {
    try {
      await clearDeviceSession();
    } finally {
      setTokens(undefined);
      setConversations([]);
      setHomeSnapshot(undefined);
      setHomeError(undefined);
      setConversationMessages([]);
      setSelectedConversationId(undefined);
      setConversationJobs({});
      setAgentAuthentication(undefined);
      pendingConversationId.current = undefined;
      await bootstrapTrustedNetworkDevice();
    }
  }

  useEffect(() => {
    let current = true;

    if (!apiBaseUrl) {
      setMode("configuration");
      setSessionLoaded(true);
      return () => {
        current = false;
      };
    }

    void readDeviceSession()
      .then(async (stored) => {
        if (!current) return;
        if (stored) {
          setTokens(stored.tokens);
          setMode("loading");
        } else {
          await bootstrapTrustedNetworkDevice();
        }
      })
      .catch(() => {
        if (current) {
          void bootstrapTrustedNetworkDevice();
        }
      })
      .finally(() => {
        if (current) setSessionLoaded(true);
      });

    return () => {
      current = false;
    };
  }, [apiBaseUrl, bootstrapTrustedNetworkDevice]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (
      !tokens ||
      !agentAuthentication ||
      !["requested", "awaiting_authorization"].includes(
        agentAuthentication.state,
      )
    ) {
      return;
    }
    let current = true;
    const poll = async () => {
      try {
        const authentication = await fetchAgentAuthentication(
          apiBaseUrl,
          tokens.accessToken,
        );
        if (current) setAgentAuthentication(authentication);
      } catch {
        if (current)
          setConversationError(copy.messages.authenticationLoadNotice);
      }
    };
    const interval = window.setInterval(() => void poll(), 1_500);
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [agentAuthentication, apiBaseUrl, tokens]);

  const activeJobIds = useMemo(
    () =>
      Object.values(conversationJobs)
        .filter((job) => !isTerminalAgentJob(job.state))
        .map((job) => job.id)
        .sort(),
    [conversationJobs],
  );
  const activeJobKey = activeJobIds.join(":");

  useEffect(() => {
    if (!tokens || activeJobIds.length === 0) return;
    let current = true;
    let polling = false;

    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const results = await Promise.all(
          activeJobIds.map(async (jobId) => {
            try {
              return await fetchAgentJob(apiBaseUrl, tokens.accessToken, jobId);
            } catch {
              return undefined;
            }
          }),
        );
        if (!current) return;
        const jobs = results.filter((job): job is AgentJob => Boolean(job));
        if (jobs.length !== activeJobIds.length) {
          setConversationError(copy.messages.conversationLoadNotice);
        }
        if (jobs.length === 0) return;
        setConversationJobs((known) => {
          const next = { ...known };
          for (const job of jobs) next[job.conversationId] = job;
          return next;
        });
        const selectedJob = jobs.find(
          (job) => job.conversationId === selectedConversationId,
        );
        if (selectedJob && selectedConversationId) {
          await loadConversationMessages(selectedConversationId, true);
        }
        const finishedConversationIds = jobs
          .filter((job) => isTerminalAgentJob(job.state))
          .map((job) => job.conversationId);
        if (finishedConversationIds.length) void refreshConversations();
      } finally {
        polling = false;
      }
    };

    void poll();
    const interval = window.setInterval(
      () => void poll(),
      ACTIVE_RESPONSE_POLL_INTERVAL_MS,
    );
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [
    activeJobKey,
    apiBaseUrl,
    loadConversationMessages,
    refreshConversations,
    selectedConversationId,
    tokens,
  ]);

  function selectConversation(conversationId: string) {
    setDestination("assistant");
    setSelectedConversationId(conversationId);
    setConversationMessages([]);
    void loadConversationMessages(conversationId);
    void restoreConversationJob(conversationId);
  }

  async function restoreConversationJob(conversationId: string) {
    if (!tokens) return;
    try {
      const job = await fetchLatestConversationJob(
        apiBaseUrl,
        tokens.accessToken,
        conversationId,
      );
      if (job) {
        setConversationJobs((known) => ({
          ...known,
          [conversationId]: job,
        }));
      }
    } catch {
      setConversationError(copy.messages.conversationLoadNotice);
    }
  }

  function startConversation() {
    setSelectedConversationId(undefined);
    setConversationMessages([]);
    setConversationError(undefined);
    pendingConversationId.current = undefined;
  }

  async function completeHomeTask(task: Task): Promise<void> {
    if (!tokens) return;
    setHomeError(undefined);
    try {
      await completeTask(apiBaseUrl, tokens.accessToken, task);
      setHomeSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
            }
          : current,
      );
    } catch {
      setHomeError(copy.messages.taskCompletionNotice);
      void loadHomeSnapshot();
    }
  }

  function openNewAssistantRequest() {
    startConversation();
    setDestination("assistant");
  }

  async function beginAgentAuthentication(): Promise<void> {
    if (!tokens || authenticationRequesting) return;
    setAuthenticationRequesting(true);
    setConversationError(undefined);
    try {
      setAgentAuthentication(
        await requestAgentAuthentication(apiBaseUrl, tokens.accessToken),
      );
    } catch {
      setConversationError(copy.messages.authenticationStartNotice);
    } finally {
      setAuthenticationRequesting(false);
    }
  }

  async function sendConversationRequest(
    text: string,
    clientMessageId: string,
  ): Promise<boolean> {
    if (!tokens || agentAuthentication?.state !== "ready") {
      setConversationError(copy.messages.authenticationRequired);
      return false;
    }
    let conversationId = selectedConversationId;
    setConversationError(undefined);
    try {
      if (!conversationId) {
        const clientConversationId =
          pendingConversationId.current ?? createUuidV7();
        pendingConversationId.current = clientConversationId;
        const conversation = await createConversation(
          apiBaseUrl,
          tokens.accessToken,
          clientConversationId,
          conversationTitle(text),
        );
        pendingConversationId.current = undefined;
        conversationId = conversation.id;
        setConversations((current) => [conversation, ...current]);
        setSelectedConversationId(conversation.id);
      }
      const queued = await queueAgentTurn(
        apiBaseUrl,
        tokens.accessToken,
        conversationId,
        text.trim(),
        clientMessageId,
      );
      setConversationJobs((known) => ({
        ...known,
        [queued.conversationId]: {
          id: queued.jobId,
          conversationId: queued.conversationId,
          state: queued.state,
          createdAt: new Date().toISOString(),
          finishedAt: null,
          version: 1,
        },
      }));
      await loadConversationMessages(queued.conversationId);
      void refreshConversations();
      return true;
    } catch (error) {
      setConversationError(
        error instanceof AgentRequestError && error.code === "conflict"
          ? copy.messages.conversationBusy
          : copy.messages.conversationSendNotice,
      );
      return false;
    }
  }

  return (
    <div className="app-shell">
      {mode === "configuration" ? (
        <main className="setup-main">
          <ServerConfigurationPanel />
        </main>
      ) : mode === "server-unreachable" ? (
        <main className="setup-main">
          <PersonalServerRecoveryPanel
            message={message ?? copy.messages.serverOffline}
            onRetry={() => void bootstrapTrustedNetworkDevice()}
          />
        </main>
      ) : (
        <OsShell
          destination={destination}
          onNavigate={setDestination}
          onRefresh={() => void refresh()}
          refreshing={mode === "loading"}
          rail={
            destination === "home" ? (
              <AssistantRail
                assistantReady={agentAuthentication?.state === "ready"}
                conversations={conversations}
                onOpenAssistant={openNewAssistantRequest}
              />
            ) : undefined
          }
        >
          {destination === "home" ? (
            <HomeWorkspace
              snapshot={homeSnapshot}
              loading={homeLoading || mode === "loading"}
              error={homeError ?? (mode === "error" ? message : undefined)}
              assistantReady={agentAuthentication?.state === "ready"}
              conversations={conversations}
              onOpenAssistant={openNewAssistantRequest}
              onCompleteTask={completeHomeTask}
            />
          ) : (
            <ConversationWorkspace
              conversations={conversations}
              messages={conversationMessages}
              selectedConversationId={selectedConversationId}
              jobState={
                selectedConversationId
                  ? conversationJobs[selectedConversationId]?.state
                  : undefined
              }
              hasActiveJob={Boolean(
                selectedConversationId &&
                conversationJobs[selectedConversationId] &&
                !isTerminalAgentJob(
                  conversationJobs[selectedConversationId].state,
                ),
              )}
              authentication={agentAuthentication}
              authenticationRequesting={authenticationRequesting}
              loading={conversationLoading}
              error={
                conversationError ?? (mode === "error" ? message : undefined)
              }
              onSelect={selectConversation}
              onStartConversation={startConversation}
              onStartAuthentication={beginAgentAuthentication}
              onSend={sendConversationRequest}
            />
          )}
        </OsShell>
      )}
    </div>
  );
}

function PersonalServerRecoveryPanel({
  message,
  onRetry,
}: {
  message: string;
  onRetry(): void;
}) {
  return (
    <section className="setup-panel" aria-labelledby="personal-server-title">
      <div className="setup-panel__intro">
        <Server aria-hidden="true" />
        <h1 id="personal-server-title">{copy.personalServer.title}</h1>
        <p className="setup-panel__description" role="alert">
          {message}
        </p>
      </div>
      <button
        className="primary-button focus-visible-control"
        type="button"
        onClick={onRetry}
      >
        {copy.actions.retryPersonalServer}
      </button>
    </section>
  );
}

function ServerConfigurationPanel() {
  return (
    <section className="setup-panel" aria-labelledby="configuration-title">
      <div className="setup-panel__intro">
        <Server aria-hidden="true" />
        <p className="setup-panel__eyebrow">{copy.configuration.eyebrow}</p>
        <h1 id="configuration-title">{copy.configuration.title}</h1>
        <p className="setup-panel__description">
          {copy.configuration.description}
        </p>
      </div>
      <aside
        className="setup-panel__scope"
        aria-label={copy.configuration.nextTitle}
      >
        <strong>{copy.configuration.nextTitle}</strong>
        <p>{copy.configuration.nextDescription}</p>
      </aside>
    </section>
  );
}
function conversationTitle(value: string) {
  const title = value.trim().replace(/\s+/g, " ").slice(0, 36);
  return title || null;
}

function currentLocalDayRange(now = new Date()): [Date, Date] {
  const from = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const to = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
  return [from, to];
}

function isTerminalAgentJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}
