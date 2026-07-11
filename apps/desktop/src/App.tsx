import { RefreshCw, ScanLine, Server } from "lucide-react";
import {
  FormEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke } from "@tauri-apps/api/core";

import {
  bootstrapLocalPhoneTestSession,
  exchangePairingCode,
  isLocalPhoneTest,
  pairingTokenFromScannedQr,
  refreshDeviceSession,
  type SessionTokens,
} from "./api/planning";
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
  | "configuration"
  | "setup"
  | "local-test-error"
  | "loading"
  | "ready"
  | "error";
type ConversationJobs = Record<string, AgentJob>;

export default function App() {
  const apiBaseUrl = personalServerBaseUrl ?? "";
  const [tokens, setTokens] = useState<SessionTokens | undefined>(undefined);
  const [sessionLoaded, setSessionLoaded] = useState(false);
  const [mode, setMode] = useState<AppMode>("loading");
  const [conversations, setConversations] = useState<Conversation[]>([]);
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

  const bootstrapLocalPhoneTestDevice = useCallback(async () => {
    setMode("loading");
    setMessage(undefined);
    try {
      const installationId = await readOrCreateInstallationId();
      const testTokens = await bootstrapLocalPhoneTestSession(
        apiBaseUrl,
        copy.localPhoneTest.deviceName,
        installationId,
      );
      await saveDeviceSession({ tokens: testTokens });
      setTokens(testTokens);
    } catch {
      setMode("local-test-error");
      setMessage(copy.messages.localPhoneTestUnavailable);
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

  const loadConversationMessages = useCallback(
    async (conversationId: string) => {
      if (!tokens) return;
      setConversationLoading(true);
      setConversationError(undefined);
      try {
        setConversationMessages(
          await fetchConversationMessages(
            apiBaseUrl,
            tokens.accessToken,
            conversationId,
          ),
        );
      } catch (error) {
        setConversationMessages([]);
        setConversationError(
          error instanceof AgentRequestError && error.code === "notFound"
            ? copy.messages.conversationChanged
            : copy.messages.conversationLoadNotice,
        );
      } finally {
        setConversationLoading(false);
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
  }, [apiBaseUrl, sessionLoaded, tokens]);

  async function discardSession() {
    try {
      await clearDeviceSession();
    } finally {
      setTokens(undefined);
      setConversations([]);
      setConversationMessages([]);
      setSelectedConversationId(undefined);
      setConversationJobs({});
      setAgentAuthentication(undefined);
      pendingConversationId.current = undefined;
      setMode("setup");
      setMessage(copy.messages.sessionExpired);
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
        } else if (isLocalPhoneTest()) {
          await bootstrapLocalPhoneTestDevice();
        } else {
          setMode("setup");
        }
      })
      .catch(() => {
        if (current) {
          setMode("setup");
          setMessage(copy.messages.storageNotice);
        }
      })
      .finally(() => {
        if (current) setSessionLoaded(true);
      });

    return () => {
      current = false;
    };
  }, [apiBaseUrl, bootstrapLocalPhoneTestDevice]);

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

    const poll = async () => {
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
      const finishedConversationIds = jobs
        .filter((job) => isTerminalAgentJob(job.state))
        .map((job) => job.conversationId);
      if (finishedConversationIds.includes(selectedConversationId ?? "")) {
        void loadConversationMessages(selectedConversationId!);
      }
      if (finishedConversationIds.length) void refreshConversations();
    };

    void poll();
    const interval = window.setInterval(() => void poll(), 1_500);
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

  async function pairDevice(pairingCode: string, deviceName: string) {
    if (!apiBaseUrl) {
      setMode("configuration");
      return;
    }
    const normalizedCode = pairingCode.trim();
    const normalizedDeviceName = deviceName.trim();
    if (!normalizedCode || !normalizedDeviceName) {
      setMessage(copy.messages.setupRequired);
      return;
    }
    setMode("loading");
    try {
      const nextTokens = await exchangePairingCode(
        apiBaseUrl,
        normalizedCode,
        normalizedDeviceName,
        await readOrCreateInstallationId(),
      );
      await saveDeviceSession({ tokens: nextTokens });
      setTokens(nextTokens);
      setMessage(undefined);
      scrollToTop();
    } catch {
      setMode("setup");
      setMessage(copy.messages.connectionNotice);
    }
  }

  function selectConversation(conversationId: string) {
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
      <header className="app-header">
        <div className="app-header__inner">
          <div className="brand">
            <span className="brand__mark" aria-hidden="true">
              J
            </span>
            <span className="brand__name">Jimin OS</span>
          </div>
          <div className="app-header__controls">
            {tokens && (
              <button
                className="quiet-button focus-visible-control"
                type="button"
                aria-label={copy.actions.refresh}
                onClick={() => void refresh()}
                disabled={mode === "loading"}
              >
                <RefreshCw aria-hidden="true" />
                <span className="refresh-label">{copy.actions.refresh}</span>
              </button>
            )}
          </div>
        </div>
      </header>
      <main
        className={
          mode === "setup" ||
          mode === "configuration" ||
          mode === "local-test-error"
            ? "setup-main"
            : "conversation-main"
        }
      >
        {mode === "configuration" ? (
          <ServerConfigurationPanel />
        ) : mode === "local-test-error" ? (
          <LocalPhoneTestRecoveryPanel
            message={message ?? copy.messages.localPhoneTestUnavailable}
            onRetry={() => void bootstrapLocalPhoneTestDevice()}
          />
        ) : mode === "setup" ? (
          <>
            {message && (
              <p className="inline-alert" role="alert">
                {message}
              </p>
            )}
            <SetupPanel onPairingCode={pairDevice} />
          </>
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
      </main>
    </div>
  );
}

function LocalPhoneTestRecoveryPanel({
  message,
  onRetry,
}: {
  message: string;
  onRetry(): void;
}) {
  return (
    <section className="setup-panel" aria-labelledby="local-test-title">
      <div className="setup-panel__intro">
        <Server aria-hidden="true" />
        <h1 id="local-test-title">{copy.localPhoneTest.title}</h1>
        <p className="setup-panel__description">{message}</p>
      </div>
      <button
        className="primary-button focus-visible-control"
        type="button"
        onClick={onRetry}
      >
        {copy.actions.retryLocalPhoneTest}
      </button>
    </section>
  );
}

function SetupPanel({
  onPairingCode,
}: {
  onPairingCode(pairingCode: string, deviceName: string): void;
}) {
  const [deviceName, setDeviceName] = useState<string>(
    copy.setup.defaultDeviceName,
  );
  const [manualCode, setManualCode] = useState("");
  const [manualEntryVisible, setManualEntryVisible] = useState(false);
  const [scannerPending, setScannerPending] = useState(false);
  const [setupNotice, setSetupNotice] = useState<string | undefined>(undefined);

  function validateDeviceName(): boolean {
    if (deviceName.trim()) return true;
    setSetupNotice(copy.messages.deviceNameRequired);
    return false;
  }

  async function startQrScan() {
    if (!validateDeviceName()) return;
    if (!("__TAURI_INTERNALS__" in window)) {
      setManualEntryVisible(true);
      setSetupNotice(copy.messages.cameraUnavailable);
      return;
    }

    setScannerPending(true);
    setSetupNotice(undefined);

    try {
      const scanned = await invoke<QrScanResponse>("scan_qr_code");
      if (!scanned.content) return;
      if (!pairingTokenFromScannedQr(scanned.content)) {
        setManualEntryVisible(true);
        setSetupNotice(copy.messages.qrCodeNeedsAnotherScan);
        return;
      }

      onPairingCode(scanned.content, deviceName);
    } catch {
      setManualEntryVisible(true);
      setSetupNotice(copy.messages.cameraUnavailable);
    } finally {
      setScannerPending(false);
    }
  }

  function submitManualCode(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!validateDeviceName()) return;
    if (!manualCode.trim()) {
      setSetupNotice(copy.messages.manualCodeRequired);
      return;
    }
    setSetupNotice(undefined);
    onPairingCode(manualCode, deviceName);
  }

  return (
    <section className="setup-panel" aria-labelledby="setup-title">
      <div className="setup-panel__intro">
        <Server aria-hidden="true" />
        <p className="setup-panel__eyebrow">{copy.setup.eyebrow}</p>
        <h1 id="setup-title">{copy.setup.title}</h1>
        <p className="setup-panel__description">{copy.setup.description}</p>
      </div>
      <aside className="setup-panel__scope" aria-label={copy.setup.scopeTitle}>
        <strong>{copy.setup.scopeTitle}</strong>
        <p>{copy.setup.scopeDescription}</p>
      </aside>
      <form className="setup-form" onSubmit={submitManualCode}>
        <div className="field">
          <label htmlFor="device-name">{copy.setup.deviceLabel}</label>
          <p id="device-name-hint" className="field__hint">
            {copy.setup.deviceHint}
          </p>
          <input
            id="device-name"
            name="deviceName"
            value={deviceName}
            maxLength={80}
            required
            aria-describedby="device-name-hint"
            onChange={(event) => setDeviceName(event.target.value)}
          />
        </div>
        <div className="setup-scan-action">
          <button
            className="primary-button focus-visible-control"
            type="button"
            onClick={() => void startQrScan()}
            disabled={scannerPending}
          >
            <ScanLine aria-hidden="true" />
            {scannerPending ? copy.actions.openingScanner : copy.actions.scanQr}
          </button>
          <p>{copy.setup.scanHint}</p>
        </div>
        {setupNotice && (
          <div className="setup-inline-alert" role="alert">
            <p>{setupNotice}</p>
          </div>
        )}
        {!manualEntryVisible ? (
          <button
            className="setup-manual-toggle focus-visible-control"
            type="button"
            onClick={() => setManualEntryVisible(true)}
          >
            {copy.actions.enterCode}
          </button>
        ) : (
          <div className="field setup-manual-entry">
            <label htmlFor="pairing-code">{copy.setup.tokenLabel}</label>
            <p id="pairing-code-hint" className="field__hint">
              {copy.setup.tokenHint}
            </p>
            <textarea
              id="pairing-code"
              name="pairingCode"
              value={manualCode}
              rows={3}
              aria-describedby="pairing-code-hint"
              onChange={(event) => setManualCode(event.target.value)}
            />
            <button
              className="secondary-button focus-visible-control"
              type="submit"
            >
              {copy.actions.connect}
            </button>
          </div>
        )}
      </form>
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

function isTerminalAgentJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}

function scrollToTop() {
  window.scrollTo(0, 0);
}

interface QrScanResponse {
  content: string | null;
}
