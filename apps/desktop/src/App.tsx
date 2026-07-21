import { Server, Sparkles } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import {
  disconnectGoogleCalendar,
  fetchGoogleCalendarConnection,
  startGoogleCalendarAuthorization,
  synchronizeGoogleCalendar,
  type GoogleCalendarConnection,
} from "./api/calendar";
import {
  bootstrapTrustedNetworkSession,
  completeTask,
  createScheduleEntry,
  createTask,
  deleteTask,
  deleteScheduleEntry,
  refreshDeviceSession,
  updateTask,
  updateScheduleEntry,
  fetchPlanning,
  type PlanningSnapshot,
  type ScheduleEntry,
  type SessionTokens,
  type Task,
} from "./api/planning";
import {
  createProject,
  deleteProject,
  fetchProjects,
  fetchProjectTasks,
  fetchWorkspaces,
  updateProject,
  type Project,
  type Workspace,
} from "./api/projects";
import { createGoal, fetchGoals, updateGoal, type Goal } from "./api/goals";
import {
  createProjectWebhook,
  deleteProjectWebhook,
  fetchProjectWebhooks,
  fetchWebhookDeliveries,
  retryWebhookDelivery,
  testProjectWebhook,
  updateProjectWebhook,
  type ManagedWebhookProvider,
  type ProjectWebhook,
  type ProjectWebhookEvent,
  type WebhookDestinationMode,
  type WebhookDelivery,
} from "./api/webhooks";
import {
  type HomeSnapshot,
  type Recommendation,
  fetchHomeSnapshot,
} from "./api/home";
import {
  decideRecommendation,
  fetchRecommendationHistory,
  refreshWorkBrief,
  type RecommendationDecision,
} from "./api/intelligence";
import { processVoiceCommand } from "./api/voice";
import { disablePushRegistration, registerFcmToken } from "./api/push";
import {
  fetchSyncChanges,
  streamSyncCursor,
  type SyncChange,
} from "./api/sync";
import {
  AgentRequestError,
  createConversation,
  fetchAgentAuthentication,
  fetchAgentModelSettings,
  fetchConversationMessages,
  fetchConversations,
  fetchLatestConversationJob,
  queueAgentTurn,
  requestAgentAuthentication,
  resolveAgentAction,
  streamConversationUpdates,
  updateAgentModelSettings,
  type AgentAuthentication,
  type AgentJob,
  type AgentModelSettings,
  type Conversation,
  type ConversationMessage,
} from "./api/agent";
import {
  assistantResponseAfterLatestRequest,
  ConversationWorkspace,
} from "./components/ConversationWorkspace";
import { DecisionInboxWorkspace } from "./components/DecisionInboxWorkspace";
import { AssistantRail, HomeWorkspace } from "./components/HomeWorkspace";
import { MemoryWorkspace } from "./components/MemoryWorkspace";
import { OsShell, type OsDestination } from "./components/OsShell";
import { PlanningWorkspace } from "./components/PlanningWorkspace";
import {
  PlanningItemEditor,
  type PlanningEditTarget,
} from "./components/PlanningItemEditor";
import { ProjectsWorkspace } from "./components/ProjectsWorkspace";
import { SettingsWorkspace } from "./components/SettingsWorkspace";
import { type VoiceCommandOutcome } from "./components/VoiceCommandSheet";
import { copy } from "./copy";
import {
  conversationIdForRequest,
  type ConversationSendOptions,
} from "./conversationRouting";
import {
  clearDeviceSession,
  readDeviceSession,
  readOrCreateInstallationId,
  saveDeviceSession,
} from "./device-session";
import { personalServerBaseUrl } from "./server-config";
import {
  isUnauthorizedFailure,
  retryUnauthorizedRequest,
} from "./session-retry";
import { createUuidV7 } from "./uuid";
import {
  earlierSyncCursor,
  laterSyncCursor,
  readSyncCursor,
  writeSyncCursor,
} from "./sync-cursor";
import {
  planningViewRange,
  samePlanningViewRange,
  type PlanningViewRange,
} from "./planningRange";
import { localDayKey, millisecondsUntilNextLocalDay } from "./homeSchedule";
import {
  acknowledgePendingReminderNavigation,
  cancelLocalReminder,
  getNativePushToken,
  getNotificationPermissionStatus,
  localNotificationsSupported,
  peekPendingReminderNavigation,
  reconcilePlanningReminders,
  reminderFallbackDestination,
  type RemoteReminderStatus,
  type ReminderSyncStatus,
} from "./local-notifications";

type AppMode =
  "configuration" | "server-unreachable" | "loading" | "ready" | "error";
type ConversationJobs = Record<string, AgentJob>;
type AssistantDraft = {
  id: string;
  text: string;
  autoSend: boolean;
};

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
  const [decisionRecommendations, setDecisionRecommendations] = useState<
    Recommendation[]
  >([]);
  const [decisionsLoading, setDecisionsLoading] = useState(false);
  const [decisionsError, setDecisionsError] = useState<string>();
  const [planningSnapshot, setPlanningSnapshot] = useState<
    PlanningSnapshot | undefined
  >();
  const [planningLoading, setPlanningLoading] = useState(false);
  const [planningError, setPlanningError] = useState<string | undefined>();
  const [planningRange, setPlanningRange] = useState<PlanningViewRange>(() =>
    planningViewRange("month"),
  );
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [projectTasks, setProjectTasks] = useState<Task[]>([]);
  const [projectWebhooks, setProjectWebhooks] = useState<ProjectWebhook[]>([]);
  const [webhookDeliveries, setWebhookDeliveries] = useState<WebhookDelivery[]>(
    [],
  );
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string>();
  const [selectedProjectId, setSelectedProjectId] = useState<string>();
  const [highlightedProjectTaskId, setHighlightedProjectTaskId] =
    useState<string>();
  const [highlightedScheduleId, setHighlightedScheduleId] = useState<string>();
  const [highlightedPlanningTaskId, setHighlightedPlanningTaskId] =
    useState<string>();
  const [planningEditTarget, setPlanningEditTarget] = useState<
    PlanningEditTarget | undefined
  >();
  const [projectsLoading, setProjectsLoading] = useState(false);
  const [webhooksLoading, setWebhooksLoading] = useState(false);
  const [projectsSaving, setProjectsSaving] = useState(false);
  const [goalsLoading, setGoalsLoading] = useState(false);
  const [goalsSaving, setGoalsSaving] = useState(false);
  const [goalsError, setGoalsError] = useState<string>();
  const [projectsError, setProjectsError] = useState<string>();
  const [workspacesReady, setWorkspacesReady] = useState(false);
  const [selectedConversationId, setSelectedConversationId] = useState<
    string | undefined
  >(undefined);
  const [homeConversationId, setHomeConversationId] = useState<
    string | undefined
  >(undefined);
  const [assistantDraft, setAssistantDraft] = useState<
    AssistantDraft | undefined
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
  const [agentModelSettings, setAgentModelSettings] = useState<
    AgentModelSettings | undefined
  >(undefined);
  const [agentModelsLoading, setAgentModelsLoading] = useState(false);
  const [agentModelsSaving, setAgentModelsSaving] = useState(false);
  const [agentModelsError, setAgentModelsError] = useState<string>();
  const [calendarConnection, setCalendarConnection] = useState<
    GoogleCalendarConnection | undefined
  >();
  const [calendarLoading, setCalendarLoading] = useState(false);
  const [calendarAction, setCalendarAction] = useState<
    "authorizing" | "syncing" | "disconnecting" | undefined
  >();
  const [calendarAuthorizationExpiresAt, setCalendarAuthorizationExpiresAt] =
    useState<string>();
  const [calendarError, setCalendarError] = useState<string>();
  const [reminderSyncStatus, setReminderSyncStatus] =
    useState<ReminderSyncStatus>("idle");
  const [reminderSyncError, setReminderSyncError] = useState<string>();
  const [remoteReminderStatus, setRemoteReminderStatus] =
    useState<RemoteReminderStatus>("idle");
  const pendingConversationId = useRef<string | undefined>(undefined);
  const openedAuthenticationUrl = useRef<string | undefined>(undefined);
  const activeSessionRef = useRef<SessionTokens | undefined>(undefined);
  const refreshInFlightRef = useRef<Promise<SessionTokens> | undefined>(
    undefined,
  );
  const syncCursorRef = useRef("0");
  const syncPullInFlightRef = useRef<Promise<void> | undefined>(undefined);
  const reminderSyncInFlightRef = useRef<Promise<boolean> | undefined>(
    undefined,
  );
  const pendingReminderInFlightRef = useRef(false);
  const [message, setMessage] = useState<string | undefined>(undefined);

  const applyActiveSession = useCallback((session: SessionTokens) => {
    activeSessionRef.current = session;
    setTokens(session);
  }, []);

  const initializeSyncCursor = useCallback((serverCursor?: string) => {
    const storedCursor = readSyncCursor();
    const cursor =
      storedCursor === undefined
        ? (serverCursor ?? "0")
        : earlierSyncCursor(storedCursor, serverCursor);
    syncCursorRef.current = cursor;
    writeSyncCursor(cursor);
  }, []);

  const persistActiveSession = useCallback(
    async (session: SessionTokens) => {
      applyActiveSession(session);
      try {
        await saveDeviceSession({ tokens: session });
      } catch {
        // The current session is still usable. A later launch will bootstrap again.
      }
    },
    [applyActiveSession],
  );

  const refreshActiveSession = useCallback(
    async (staleRefreshToken: string): Promise<SessionTokens> => {
      const current = activeSessionRef.current;
      if (current && current.refreshToken !== staleRefreshToken) return current;
      if (refreshInFlightRef.current) return refreshInFlightRef.current;

      const refresh = refreshDeviceSession(apiBaseUrl, staleRefreshToken).then(
        async (refreshed) => {
          await persistActiveSession(refreshed.tokens);
          return refreshed.tokens;
        },
      );
      refreshInFlightRef.current = refresh;
      try {
        return await refresh;
      } finally {
        if (refreshInFlightRef.current === refresh) {
          refreshInFlightRef.current = undefined;
        }
      }
    },
    [apiBaseUrl, persistActiveSession],
  );

  const withAuthenticatedSession = useCallback(
    async <T,>(operation: (accessToken: string) => Promise<T>): Promise<T> => {
      const session = activeSessionRef.current;
      if (!session) throw new AgentRequestError("unauthorized");
      return retryUnauthorizedRequest(session, operation, refreshActiveSession);
    },
    [refreshActiveSession],
  );

  const bootstrapTrustedNetworkDevice = useCallback(async () => {
    setMode("loading");
    setMessage(undefined);
    try {
      const installationId = await readOrCreateInstallationId();
      const issued = await bootstrapTrustedNetworkSession(
        apiBaseUrl,
        copy.personalServer.deviceName,
        installationId,
      );
      initializeSyncCursor(issued.syncCursor);
      await persistActiveSession(issued.tokens);
    } catch {
      setMode("server-unreachable");
      setMessage(copy.messages.serverOffline);
    }
  }, [apiBaseUrl, initializeSyncCursor, persistActiveSession]);

  const refreshConversations = useCallback(async () => {
    if (!tokens) return;
    setConversationLoading(true);
    setConversationError(undefined);
    try {
      setConversations(
        await withAuthenticatedSession((accessToken) =>
          fetchConversations(apiBaseUrl, accessToken),
        ),
      );
    } catch {
      setConversationError(copy.messages.conversationLoadNotice);
    } finally {
      setConversationLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadHomeSnapshot = useCallback(async () => {
    if (!tokens) return undefined;
    setHomeLoading(true);
    setHomeError(undefined);
    try {
      const [from, to] = currentLocalDayRange();
      await withAuthenticatedSession((accessToken) =>
        refreshWorkBrief(apiBaseUrl, accessToken),
      ).catch(() => undefined);
      const snapshot = await withAuthenticatedSession((accessToken) =>
        fetchHomeSnapshot(apiBaseUrl, accessToken, from, to),
      );
      setHomeSnapshot(snapshot);
      return snapshot;
    } catch {
      setHomeError(copy.messages.homeLoadNotice);
      return undefined;
    } finally {
      setHomeLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadDecisionInbox = useCallback(async () => {
    if (!tokens) return;
    setDecisionsLoading(true);
    setDecisionsError(undefined);
    try {
      const items = await withAuthenticatedSession((accessToken) =>
        fetchRecommendationHistory(apiBaseUrl, accessToken),
      );
      setDecisionRecommendations(items);
    } catch {
      setDecisionsError(copy.decisions.loadNotice);
    } finally {
      setDecisionsLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadPlanningSnapshot = useCallback(
    async (targetStartsAt?: string, requestedRange?: PlanningViewRange) => {
      if (!tokens) return undefined;
      setPlanningLoading(true);
      setPlanningError(undefined);
      try {
        const targetDate = targetStartsAt
          ? new Date(targetStartsAt)
          : undefined;
        const nextRange =
          requestedRange ??
          (targetDate && !Number.isNaN(targetDate.getTime())
            ? planningViewRange("month", targetDate)
            : planningRange);
        setPlanningRange((current) =>
          samePlanningViewRange(current, nextRange) ? current : nextRange,
        );
        const snapshot = await withAuthenticatedSession((accessToken) =>
          fetchPlanning(apiBaseUrl, accessToken, nextRange.from, nextRange.to),
        );
        setPlanningSnapshot(snapshot);
        return snapshot;
      } catch {
        setPlanningError(copy.messages.homeLoadNotice);
        return undefined;
      } finally {
        setPlanningLoading(false);
      }
    },
    [apiBaseUrl, planningRange, tokens, withAuthenticatedSession],
  );

  const changePlanningRange = useCallback(
    async (range: PlanningViewRange): Promise<void> => {
      await loadPlanningSnapshot(undefined, range);
    },
    [loadPlanningSnapshot],
  );

  const synchronizePlanningReminders =
    useCallback(async (): Promise<boolean> => {
      if (!tokens || !localNotificationsSupported()) return false;
      if (reminderSyncInFlightRef.current) {
        return reminderSyncInFlightRef.current;
      }
      const operation = (async () => {
        setReminderSyncStatus("syncing");
        setReminderSyncError(undefined);
        setRemoteReminderStatus("syncing");
        try {
          const [from, to] = currentReminderRange();
          const snapshot = await withAuthenticatedSession((accessToken) =>
            fetchPlanning(apiBaseUrl, accessToken, from, to),
          );
          await reconcilePlanningReminders(snapshot);
          const permission = await getNotificationPermissionStatus();
          if (permission.status === "granted") {
            const pushToken = await getNativePushToken();
            if (pushToken.state === "ready") {
              try {
                await withAuthenticatedSession((accessToken) =>
                  registerFcmToken(
                    apiBaseUrl,
                    accessToken,
                    pushToken.registrationHandle,
                  ),
                );
                setRemoteReminderStatus("connected");
              } catch {
                setRemoteReminderStatus("error");
              }
            } else {
              setRemoteReminderStatus(
                pushToken.state === "unconfigured" ? "local-only" : "error",
              );
            }
          } else {
            try {
              await withAuthenticatedSession((accessToken) =>
                disablePushRegistration(apiBaseUrl, accessToken),
              );
            } catch {
              // Registration cleanup is retried on the next reconciliation.
            }
            setRemoteReminderStatus("local-only");
          }
          setReminderSyncStatus("ready");
          return true;
        } catch {
          setReminderSyncStatus("error");
          setReminderSyncError(copy.settings.notificationsSyncNotice);
          setRemoteReminderStatus("error");
          return false;
        }
      })();
      reminderSyncInFlightRef.current = operation;
      try {
        return await operation;
      } finally {
        if (reminderSyncInFlightRef.current === operation) {
          reminderSyncInFlightRef.current = undefined;
        }
      }
    }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadAgentModelSettings = useCallback(async () => {
    if (!tokens) return;
    setAgentModelsLoading(true);
    setAgentModelsError(undefined);
    try {
      setAgentModelSettings(
        await withAuthenticatedSession((accessToken) =>
          fetchAgentModelSettings(apiBaseUrl, accessToken),
        ),
      );
    } catch {
      setAgentModelsError(copy.settings.modelLoadFailed);
    } finally {
      setAgentModelsLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const saveAgentModelSettings = useCallback(
    async (
      modelId: string | null,
      reasoningEffort: string | null,
    ): Promise<boolean> => {
      if (!tokens) return false;
      setAgentModelsSaving(true);
      setAgentModelsError(undefined);
      try {
        setAgentModelSettings(
          await withAuthenticatedSession((accessToken) =>
            updateAgentModelSettings(
              apiBaseUrl,
              accessToken,
              modelId,
              reasoningEffort,
            ),
          ),
        );
        return true;
      } catch {
        setAgentModelsError(copy.settings.modelSaveFailed);
        return false;
      } finally {
        setAgentModelsSaving(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadGoogleCalendarConnection = useCallback(async (): Promise<
    GoogleCalendarConnection | undefined
  > => {
    if (!tokens) return undefined;
    setCalendarLoading(true);
    setCalendarError(undefined);
    try {
      const connection = await withAuthenticatedSession((accessToken) =>
        fetchGoogleCalendarConnection(apiBaseUrl, accessToken),
      );
      setCalendarConnection(connection);
      if (connection.status === "active") {
        setCalendarAuthorizationExpiresAt(undefined);
      }
      return connection;
    } catch {
      setCalendarError(copy.settings.calendarLoadFailed);
      return undefined;
    } finally {
      setCalendarLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const beginGoogleCalendarConnection = useCallback(async (): Promise<void> => {
    if (!tokens || calendarAction) return;
    setCalendarAction("authorizing");
    setCalendarError(undefined);
    try {
      const authorization = await withAuthenticatedSession((accessToken) =>
        startGoogleCalendarAuthorization(apiBaseUrl, accessToken),
      );
      await openExternalUrl(authorization.authorizationUrl);
      setCalendarAuthorizationExpiresAt(authorization.expiresAt);
    } catch {
      setCalendarError(
        calendarConnection?.available === false
          ? copy.settings.calendarConfigurationMissing
          : copy.settings.calendarConnectFailed,
      );
    } finally {
      setCalendarAction(undefined);
    }
  }, [
    apiBaseUrl,
    calendarAction,
    calendarConnection?.available,
    tokens,
    withAuthenticatedSession,
  ]);

  const syncGoogleCalendar = useCallback(async (): Promise<void> => {
    if (!tokens || calendarAction) return;
    setCalendarAction("syncing");
    setCalendarError(undefined);
    try {
      const connection = await withAuthenticatedSession((accessToken) =>
        synchronizeGoogleCalendar(apiBaseUrl, accessToken),
      );
      setCalendarConnection(connection);
      await Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
    } catch {
      setCalendarError(copy.settings.calendarSyncFailed);
    } finally {
      setCalendarAction(undefined);
    }
  }, [
    apiBaseUrl,
    calendarAction,
    loadHomeSnapshot,
    loadPlanningSnapshot,
    tokens,
    withAuthenticatedSession,
  ]);

  const disconnectGoogleCalendarConnection =
    useCallback(async (): Promise<boolean> => {
      const expectedVersion = calendarConnection?.version;
      if (
        !tokens ||
        calendarAction ||
        expectedVersion === null ||
        expectedVersion === undefined
      ) {
        return false;
      }
      setCalendarAction("disconnecting");
      setCalendarError(undefined);
      try {
        await withAuthenticatedSession((accessToken) =>
          disconnectGoogleCalendar(apiBaseUrl, accessToken, expectedVersion),
        );
        setCalendarAuthorizationExpiresAt(undefined);
        await Promise.all([
          loadGoogleCalendarConnection(),
          loadHomeSnapshot(),
          loadPlanningSnapshot(),
        ]);
        return true;
      } catch {
        setCalendarError(copy.settings.calendarDisconnectProblem);
        return false;
      } finally {
        setCalendarAction(undefined);
      }
    }, [
      apiBaseUrl,
      calendarAction,
      calendarConnection?.version,
      loadGoogleCalendarConnection,
      loadHomeSnapshot,
      loadPlanningSnapshot,
      tokens,
      withAuthenticatedSession,
    ]);

  const loadWorkspaces = useCallback(async () => {
    if (!tokens) return;
    setWorkspacesReady(false);
    setProjectsLoading(true);
    setProjectsError(undefined);
    try {
      const items = await withAuthenticatedSession((accessToken) =>
        fetchWorkspaces(apiBaseUrl, accessToken),
      );
      setWorkspaces(items);
      setSelectedWorkspaceId((current) =>
        items.some((workspace) => workspace.id === current)
          ? current
          : items[0]?.id,
      );
      setWorkspacesReady(true);
    } catch {
      setWorkspacesReady(false);
      setProjectsError(copy.messages.projectsLoadNotice);
    } finally {
      setProjectsLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadGoals = useCallback(async () => {
    if (!tokens) return;
    setGoalsLoading(true);
    setGoalsError(undefined);
    try {
      setGoals(
        await withAuthenticatedSession((accessToken) =>
          fetchGoals(apiBaseUrl, accessToken),
        ),
      );
    } catch {
      setGoalsError(copy.goals.loadProblem);
    } finally {
      setGoalsLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const loadProjectsForWorkspace = useCallback(
    async (workspaceId: string, preferredProjectId?: string) => {
      if (!tokens) return false;
      setProjectsLoading(true);
      setProjectsError(undefined);
      try {
        const items = await withAuthenticatedSession((accessToken) =>
          fetchProjects(apiBaseUrl, accessToken, workspaceId),
        );
        setProjects(items);
        setSelectedProjectId((current) => {
          const next = preferredProjectId ?? current;
          return items.some((project) => project.id === next)
            ? next
            : undefined;
        });
        return true;
      } catch {
        setProjects([]);
        setSelectedProjectId(undefined);
        setProjectTasks([]);
        setProjectsError(copy.messages.projectsLoadNotice);
        return false;
      } finally {
        setProjectsLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadProjectTasks = useCallback(
    async (projectId: string) => {
      if (!tokens) return undefined;
      setProjectsLoading(true);
      try {
        const items = await withAuthenticatedSession((accessToken) =>
          fetchProjectTasks(apiBaseUrl, accessToken, projectId),
        );
        setProjectTasks(items);
        return items;
      } catch {
        setProjectTasks([]);
        setProjectsError(copy.messages.projectsLoadNotice);
        return undefined;
      } finally {
        setProjectsLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadProjectWebhooks = useCallback(
    async (projectId: string) => {
      if (!tokens) return undefined;
      setWebhooksLoading(true);
      try {
        const [webhooks, deliveries] = await withAuthenticatedSession(
          (accessToken) =>
            Promise.all([
              fetchProjectWebhooks(apiBaseUrl, accessToken, projectId),
              fetchWebhookDeliveries(apiBaseUrl, accessToken, projectId),
            ]),
        );
        setProjectWebhooks(webhooks);
        setWebhookDeliveries(deliveries);
        return { webhooks, deliveries };
      } catch {
        setProjectWebhooks([]);
        setWebhookDeliveries([]);
        setProjectsError(copy.projects.webhookLoadProblem);
        return undefined;
      } finally {
        setWebhooksLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadConversationMessages = useCallback(
    async (conversationId: string, background = false) => {
      if (!tokens) return;
      if (!background) {
        setConversationLoading(true);
        setConversationError(undefined);
      }
      try {
        setConversationMessages(
          await withAuthenticatedSession((accessToken) =>
            fetchConversationMessages(apiBaseUrl, accessToken, conversationId),
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
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const refresh = useCallback(async () => {
    if (!sessionLoaded) return;
    if (!tokens) return;
    setMode("loading");
    setMessage(undefined);
    try {
      const [nextConversations, authentication] = await Promise.all([
        withAuthenticatedSession((accessToken) =>
          fetchConversations(apiBaseUrl, accessToken),
        ),
        withAuthenticatedSession((accessToken) =>
          fetchAgentAuthentication(apiBaseUrl, accessToken),
        ),
        loadHomeSnapshot(),
        loadGoogleCalendarConnection(),
      ]);
      setConversations(nextConversations);
      setAgentAuthentication(authentication);
      setMode("ready");
    } catch (error) {
      if (isUnauthorizedFailure(error)) {
        await discardSession();
        return;
      }
      setMode("error");
      setMessage(copy.messages.conversationLoadNotice);
    }
  }, [
    loadGoogleCalendarConnection,
    loadHomeSnapshot,
    sessionLoaded,
    tokens,
    withAuthenticatedSession,
  ]);

  const refreshSynchronizedProjections = useCallback(
    async (changes: SyncChange[], forceFull = false): Promise<void> => {
      const entityTypes = new Set(changes.map((change) => change.entityType));
      const affectsWork =
        forceFull ||
        [
          "task",
          "schedule_entry",
          "calendar_event",
          "calendar_account",
          "project",
          "goal",
          "intelligence_signal",
          "recommendation",
          "recommendation_decision",
          "recommendation_action_result",
        ].some((entityType) => entityTypes.has(entityType));
      const affectsDecisions =
        forceFull ||
        [
          "intelligence_signal",
          "recommendation",
          "recommendation_decision",
          "recommendation_action_result",
        ].some((entityType) => entityTypes.has(entityType));
      const affectsConversations =
        forceFull ||
        ["conversation", "message", "agent_job"].some((entityType) =>
          entityTypes.has(entityType),
        );
      const affectsAgentSettings =
        forceFull || entityTypes.has("agent_preference");
      const affectsCalendarConnection =
        forceFull || entityTypes.has("calendar_account");

      if (affectsWork) {
        const [from, to] = currentLocalDayRange();
        const synchronized = await withAuthenticatedSession(
          async (accessToken) => {
            await refreshWorkBrief(apiBaseUrl, accessToken).catch(
              () => undefined,
            );
            const [home, planning, synchronizedGoals, synchronizedProjects] =
              await Promise.all([
                fetchHomeSnapshot(apiBaseUrl, accessToken, from, to),
                fetchPlanning(
                  apiBaseUrl,
                  accessToken,
                  planningRange.from,
                  planningRange.to,
                ),
                fetchGoals(apiBaseUrl, accessToken),
                selectedWorkspaceId
                  ? fetchProjects(apiBaseUrl, accessToken, selectedWorkspaceId)
                  : Promise.resolve(undefined),
              ]);
            const synchronizedProjectTasks = selectedProjectId
              ? await fetchProjectTasks(
                  apiBaseUrl,
                  accessToken,
                  selectedProjectId,
                )
              : undefined;
            return {
              home,
              planning,
              synchronizedGoals,
              synchronizedProjects,
              synchronizedProjectTasks,
            };
          },
        );
        setHomeSnapshot(synchronized.home);
        setPlanningSnapshot(synchronized.planning);
        setGoals(synchronized.synchronizedGoals);
        if (synchronized.synchronizedProjects) {
          setProjects(synchronized.synchronizedProjects);
          if (
            selectedProjectId &&
            !synchronized.synchronizedProjects.some(
              (project) => project.id === selectedProjectId,
            )
          ) {
            setSelectedProjectId(undefined);
            setProjectTasks([]);
          } else if (synchronized.synchronizedProjectTasks) {
            setProjectTasks(synchronized.synchronizedProjectTasks);
          }
        }
      }

      if (affectsDecisions) {
        setDecisionRecommendations(
          await withAuthenticatedSession((accessToken) =>
            fetchRecommendationHistory(apiBaseUrl, accessToken),
          ),
        );
      }
      if (affectsConversations) {
        const synchronizedConversations = await withAuthenticatedSession(
          (accessToken) => fetchConversations(apiBaseUrl, accessToken),
        );
        setConversations(synchronizedConversations);
        if (selectedConversationId) {
          setConversationMessages(
            await withAuthenticatedSession((accessToken) =>
              fetchConversationMessages(
                apiBaseUrl,
                accessToken,
                selectedConversationId,
              ),
            ),
          );
        }
      }
      if (affectsAgentSettings) {
        setAgentModelSettings(
          await withAuthenticatedSession((accessToken) =>
            fetchAgentModelSettings(apiBaseUrl, accessToken),
          ),
        );
      }
      if (affectsCalendarConnection) {
        setCalendarConnection(
          await withAuthenticatedSession((accessToken) =>
            fetchGoogleCalendarConnection(apiBaseUrl, accessToken),
          ),
        );
      }
    },
    [
      apiBaseUrl,
      planningRange.from,
      planningRange.to,
      selectedConversationId,
      selectedProjectId,
      selectedWorkspaceId,
      withAuthenticatedSession,
    ],
  );

  const pullSyncChanges = useCallback(async (): Promise<void> => {
    if (!tokens) return;
    if (syncPullInFlightRef.current) return syncPullInFlightRef.current;

    const operation = (async () => {
      for (let pageNumber = 0; pageNumber < 20; pageNumber += 1) {
        const after = syncCursorRef.current;
        const page = await withAuthenticatedSession((accessToken) =>
          fetchSyncChanges(apiBaseUrl, accessToken, after),
        );
        if (BigInt(page.currentCursor) < BigInt(after)) {
          await refreshSynchronizedProjections([], true);
          syncCursorRef.current = page.currentCursor;
          writeSyncCursor(page.currentCursor);
          return;
        }
        if (page.items.length === 0) return;

        await refreshSynchronizedProjections(page.items);
        const appliedCursor = laterSyncCursor(after, page.nextCursor);
        syncCursorRef.current = appliedCursor;
        writeSyncCursor(appliedCursor);
        if (!page.hasMore) return;
      }
    })();
    syncPullInFlightRef.current = operation;
    try {
      await operation;
    } finally {
      if (syncPullInFlightRef.current === operation) {
        syncPullInFlightRef.current = undefined;
      }
    }
  }, [
    apiBaseUrl,
    refreshSynchronizedProjections,
    tokens,
    withAuthenticatedSession,
  ]);

  async function discardSession() {
    try {
      await clearDeviceSession();
    } finally {
      activeSessionRef.current = undefined;
      setTokens(undefined);
      setConversations([]);
      setHomeSnapshot(undefined);
      setHomeError(undefined);
      setPlanningSnapshot(undefined);
      setPlanningError(undefined);
      setWorkspaces([]);
      setWorkspacesReady(false);
      setProjects([]);
      setGoals([]);
      setProjectTasks([]);
      setSelectedWorkspaceId(undefined);
      setSelectedProjectId(undefined);
      setHighlightedProjectTaskId(undefined);
      setHighlightedScheduleId(undefined);
      setHighlightedPlanningTaskId(undefined);
      setPlanningEditTarget(undefined);
      setProjectsError(undefined);
      setGoalsError(undefined);
      setConversationMessages([]);
      setSelectedConversationId(undefined);
      setHomeConversationId(undefined);
      setAssistantDraft(undefined);
      setConversationJobs({});
      setAgentAuthentication(undefined);
      setAgentModelSettings(undefined);
      setAgentModelsError(undefined);
      setCalendarConnection(undefined);
      setCalendarError(undefined);
      setCalendarAuthorizationExpiresAt(undefined);
      setCalendarAction(undefined);
      setReminderSyncStatus("idle");
      setReminderSyncError(undefined);
      setRemoteReminderStatus("idle");
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
          initializeSyncCursor();
          applyActiveSession(stored.tokens);
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
  }, [
    apiBaseUrl,
    applyActiveSession,
    bootstrapTrustedNetworkDevice,
    initializeSyncCursor,
  ]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!tokens) return;
    let active = true;
    const controller = new AbortController();
    let reconnectDelay = 1_000;

    const pullVisibleChanges = () => {
      if (document.visibilityState === "visible") {
        void pullSyncChanges().catch(() => undefined);
      }
    };
    const subscribe = async () => {
      while (active && !controller.signal.aborted) {
        try {
          await withAuthenticatedSession((accessToken) =>
            streamSyncCursor(
              apiBaseUrl,
              accessToken,
              syncCursorRef.current,
              controller.signal,
              () => void pullSyncChanges().catch(() => undefined),
            ),
          );
          reconnectDelay = 1_000;
        } catch (error) {
          if (
            !active ||
            controller.signal.aborted ||
            (error instanceof DOMException && error.name === "AbortError")
          ) {
            return;
          }
        }
        await new Promise((resolve) =>
          window.setTimeout(resolve, reconnectDelay),
        );
        reconnectDelay = Math.min(reconnectDelay * 2, 15_000);
      }
    };

    void pullSyncChanges().catch(() => undefined);
    void subscribe();
    const reconciliation = window.setInterval(pullVisibleChanges, 15_000);
    window.addEventListener("focus", pullVisibleChanges);
    document.addEventListener("visibilitychange", pullVisibleChanges);
    window.addEventListener("online", pullVisibleChanges);
    return () => {
      active = false;
      controller.abort();
      window.clearInterval(reconciliation);
      window.removeEventListener("focus", pullVisibleChanges);
      document.removeEventListener("visibilitychange", pullVisibleChanges);
      window.removeEventListener("online", pullVisibleChanges);
    };
  }, [apiBaseUrl, pullSyncChanges, tokens, withAuthenticatedSession]);

  useLayoutEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      window.scrollTo({
        top: 0,
        left: 0,
        behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
          ? "auto"
          : "smooth",
      });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [destination]);

  useEffect(() => {
    void synchronizePlanningReminders();
  }, [planningSnapshot, synchronizePlanningReminders]);

  useEffect(() => {
    if (!tokens) return;
    let active = true;
    const openPendingReminder = () => {
      if (pendingReminderInFlightRef.current) return;
      pendingReminderInFlightRef.current = true;
      void peekPendingReminderNavigation()
        .then(async (navigation) => {
          if (!active || !navigation) return;
          if (
            navigation.destination === "projects" &&
            navigation.itemType === "task" &&
            navigation.projectId
          ) {
            if (!workspacesReady) return;
            await openTaskFromAssistant({
              id: navigation.itemId,
              projectId: navigation.projectId,
            });
            if (!active) return;
            await acknowledgePendingReminderNavigation(navigation);
            return;
          }
          const snapshot = await loadPlanningSnapshot(
            undefined,
            planningViewRange(
              "month",
              navigation.targetAtEpochMillis
                ? new Date(navigation.targetAtEpochMillis)
                : new Date(),
            ),
          );
          if (!active || !snapshot) return;
          setDestination(reminderFallbackDestination(navigation));
          if (navigation.itemType === "schedule") {
            setHighlightedPlanningTaskId(undefined);
            setHighlightedScheduleId(navigation.itemId);
          } else {
            setHighlightedScheduleId(undefined);
            setHighlightedPlanningTaskId(navigation.itemId);
          }
          await acknowledgePendingReminderNavigation(navigation);
        })
        .catch(() => undefined)
        .finally(() => {
          pendingReminderInFlightRef.current = false;
        });
    };
    openPendingReminder();
    window.addEventListener("focus", openPendingReminder);
    const openPendingVisibleReminder = () => {
      if (document.visibilityState === "visible") openPendingReminder();
    };
    document.addEventListener("visibilitychange", openPendingVisibleReminder);
    return () => {
      active = false;
      window.removeEventListener("focus", openPendingReminder);
      document.removeEventListener(
        "visibilitychange",
        openPendingVisibleReminder,
      );
    };
  }, [loadPlanningSnapshot, projects, tokens, workspaces, workspacesReady]);

  useEffect(() => {
    if (!tokens) return;
    let active = true;
    let observedDay = localDayKey();
    let rolloverTimer: number | undefined;

    const scheduleRollover = () => {
      rolloverTimer = window.setTimeout(() => {
        if (!active) return;
        observedDay = localDayKey();
        void loadHomeSnapshot();
        scheduleRollover();
      }, millisecondsUntilNextLocalDay());
    };
    const refreshAfterDayChange = () => {
      const currentDay = localDayKey();
      if (
        document.visibilityState !== "visible" ||
        currentDay === observedDay
      ) {
        return;
      }
      observedDay = currentDay;
      void loadHomeSnapshot();
    };

    scheduleRollover();
    document.addEventListener("visibilitychange", refreshAfterDayChange);
    return () => {
      active = false;
      if (rolloverTimer !== undefined) {
        window.clearTimeout(rolloverTimer);
      }
      document.removeEventListener("visibilitychange", refreshAfterDayChange);
    };
  }, [loadHomeSnapshot, tokens]);

  useEffect(() => {
    void loadAgentModelSettings();
  }, [loadAgentModelSettings]);

  useEffect(() => {
    if (!tokens || !calendarAuthorizationExpiresAt) return;
    let current = true;
    const expiresAt = new Date(calendarAuthorizationExpiresAt).getTime();
    const poll = async () => {
      if (!Number.isFinite(expiresAt) || Date.now() >= expiresAt) {
        if (current) {
          setCalendarAuthorizationExpiresAt(undefined);
          setCalendarError(copy.settings.calendarAuthorizationExpired);
        }
        return;
      }
      try {
        const connection = await withAuthenticatedSession((accessToken) =>
          fetchGoogleCalendarConnection(apiBaseUrl, accessToken),
        );
        if (!current) return;
        setCalendarConnection(connection);
        if (connection.status === "active") {
          setCalendarAuthorizationExpiresAt(undefined);
          setCalendarError(undefined);
          void loadHomeSnapshot();
          void loadPlanningSnapshot();
        }
      } catch {
        if (current) setCalendarError(copy.settings.calendarLoadFailed);
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), 2_000);
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [
    apiBaseUrl,
    calendarAuthorizationExpiresAt,
    loadHomeSnapshot,
    loadPlanningSnapshot,
    tokens,
    withAuthenticatedSession,
  ]);

  useEffect(() => {
    void loadWorkspaces();
  }, [loadWorkspaces]);

  useEffect(() => {
    void loadGoals();
  }, [loadGoals]);

  useEffect(() => {
    if (selectedWorkspaceId) {
      void loadProjectsForWorkspace(selectedWorkspaceId);
    }
  }, [loadProjectsForWorkspace, selectedWorkspaceId]);

  useEffect(() => {
    if (selectedProjectId) {
      void loadProjectTasks(selectedProjectId);
      void loadProjectWebhooks(selectedProjectId);
    } else {
      setProjectTasks([]);
      setProjectWebhooks([]);
      setWebhookDeliveries([]);
    }
  }, [loadProjectTasks, loadProjectWebhooks, selectedProjectId]);

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
        const authentication = await withAuthenticatedSession((accessToken) =>
          fetchAgentAuthentication(apiBaseUrl, accessToken),
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
  }, [agentAuthentication, apiBaseUrl, tokens, withAuthenticatedSession]);

  useEffect(() => {
    const verificationUrl = agentAuthentication?.verificationUrl;
    if (
      agentAuthentication?.state !== "awaiting_authorization" ||
      !verificationUrl ||
      openedAuthenticationUrl.current === verificationUrl
    ) {
      return;
    }
    openedAuthenticationUrl.current = verificationUrl;
    void openExternalUrl(verificationUrl).catch(() => {
      setConversationError(copy.authentication.browserOpenFailed);
    });
  }, [agentAuthentication]);

  const synchronizeAssistantDestinations = useCallback(
    async (messages: ConversationMessage[]): Promise<void> => {
      const presentation = [...messages]
        .reverse()
        .find(
          (candidate) =>
            candidate.role === "assistant" && candidate.status === "completed",
        )?.presentation;
      if (!presentation) return;
      const project = [...presentation.items]
        .reverse()
        .find((item) => item.type === "project");
      const schedule = [...presentation.items]
        .reverse()
        .find((item) => item.type === "schedule");
      await Promise.all([
        project
          ? loadProjectsForWorkspace(project.workspaceId, project.id).then(
              (loaded) => {
                if (loaded) setSelectedWorkspaceId(project.workspaceId);
              },
            )
          : Promise.resolve(),
        schedule ? loadPlanningSnapshot(schedule.startsAt) : Promise.resolve(),
      ]);
    },
    [loadPlanningSnapshot, loadProjectsForWorkspace],
  );

  const activeJobs = useMemo(
    () =>
      Object.values(conversationJobs)
        .filter((job) => !isTerminalAgentJob(job.state))
        .sort((left, right) => left.id.localeCompare(right.id)),
    [conversationJobs],
  );
  const activeJobKey = activeJobs
    .map((job) => `${job.conversationId}:${job.id}`)
    .join(":");

  useEffect(() => {
    if (!tokens || activeJobs.length === 0) return;
    let current = true;
    const controller = new AbortController();
    const subscribe = async (job: AgentJob) => {
      try {
        await withAuthenticatedSession((accessToken) =>
          streamConversationUpdates(
            apiBaseUrl,
            accessToken,
            job.conversationId,
            controller.signal,
            (snapshot) => {
              if (!current) return;
              const streamedJob = snapshot.job;
              if (streamedJob) {
                setConversationJobs((known) => ({
                  ...known,
                  [streamedJob.conversationId]: streamedJob,
                }));
              }
              if (job.conversationId === selectedConversationId) {
                setConversationMessages(snapshot.messages);
              }
              if (streamedJob && isTerminalAgentJob(streamedJob.state)) {
                void refreshConversations();
                void loadHomeSnapshot();
                void loadPlanningSnapshot();
                if (selectedWorkspaceId) {
                  void loadProjectsForWorkspace(
                    selectedWorkspaceId,
                    selectedProjectId,
                  );
                }
                if (selectedProjectId) {
                  void loadProjectTasks(selectedProjectId);
                }
                void synchronizeAssistantDestinations(snapshot.messages);
              }
            },
          ),
        );
      } catch (error) {
        if (
          current &&
          !(error instanceof DOMException && error.name === "AbortError")
        ) {
          setConversationError(copy.messages.conversationLoadNotice);
        }
      }
    };
    for (const job of activeJobs) void subscribe(job);
    return () => {
      current = false;
      controller.abort();
    };
  }, [
    activeJobKey,
    apiBaseUrl,
    loadConversationMessages,
    loadHomeSnapshot,
    loadPlanningSnapshot,
    loadProjectTasks,
    loadProjectsForWorkspace,
    refreshConversations,
    selectedConversationId,
    selectedProjectId,
    selectedWorkspaceId,
    synchronizeAssistantDestinations,
    tokens,
    withAuthenticatedSession,
  ]);

  function selectConversation(conversationId: string) {
    setDestination("chat");
    setAssistantDraft(undefined);
    setSelectedConversationId(conversationId);
    setConversationMessages([]);
    void loadConversationMessages(conversationId);
    void restoreConversationJob(conversationId);
  }

  async function restoreConversationJob(conversationId: string) {
    if (!tokens) return;
    try {
      const job = await withAuthenticatedSession((accessToken) =>
        fetchLatestConversationJob(apiBaseUrl, accessToken, conversationId),
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

  function startHomeConversation() {
    setHomeConversationId(undefined);
    startConversation();
  }

  function openHomeAssistant() {
    if (!homeConversationId) {
      openNewAssistantRequest();
      return;
    }
    setAssistantDraft(undefined);
    setDestination("chat");
    if (selectedConversationId !== homeConversationId) {
      setSelectedConversationId(homeConversationId);
      setConversationMessages([]);
      void loadConversationMessages(homeConversationId);
    }
    void restoreConversationJob(homeConversationId);
  }

  async function decideHomeRecommendation(
    recommendation: Recommendation,
    decision: RecommendationDecision,
  ): Promise<boolean> {
    if (!tokens) return false;
    setHomeError(undefined);
    try {
      const revisitAt =
        decision === "defer"
          ? new Date(Date.now() + 4 * 60 * 60 * 1_000).toISOString()
          : undefined;
      const updated = await withAuthenticatedSession((accessToken) =>
        decideRecommendation(
          apiBaseUrl,
          accessToken,
          recommendation,
          decision,
          revisitAt,
        ),
      );
      setHomeSnapshot((current) =>
        current
          ? {
              ...current,
              recommendations: current.recommendations.filter(
                (item) => item.id !== recommendation.id,
              ),
            }
          : current,
      );
      setDecisionRecommendations((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      return true;
    } catch {
      setHomeError(copy.messages.recommendationDecisionNotice);
      return false;
    }
  }

  async function completeHomeTask(task: Task): Promise<void> {
    if (!tokens) return;
    setHomeError(undefined);
    try {
      const completed = await withAuthenticatedSession((accessToken) =>
        completeTask(apiBaseUrl, accessToken, task),
      );
      await cancelLocalReminder("task", task.id).catch(() => false);
      setHomeSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
            }
          : current,
      );
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
              completedTasks: [
                completed,
                ...current.completedTasks.filter(
                  (item) => item.id !== completed.id,
                ),
              ],
            }
          : current,
      );
      setProjectTasks((current) =>
        current.map((item) => (item.id === completed.id ? completed : item)),
      );
      if (task.projectId) {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? {
                  ...project,
                  openTaskCount: Math.max(0, project.openTaskCount - 1),
                }
              : project,
          ),
        );
      }
      void loadGoals();
    } catch {
      setHomeError(copy.messages.taskCompletionNotice);
      setPlanningError(copy.messages.taskCompletionNotice);
      void loadHomeSnapshot();
      void loadPlanningSnapshot();
    }
  }

  async function restorePlanningTask(task: Task): Promise<void> {
    if (!tokens) return;
    setPlanningError(undefined);
    try {
      const restored = await withAuthenticatedSession((accessToken) =>
        updateTask(apiBaseUrl, accessToken, task, {
          title: task.title,
          notes: task.notes ?? undefined,
          status: "open",
          priority: task.priority,
          dueAt: task.dueAt ?? undefined,
        }),
      );
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: [
                restored,
                ...current.tasks.filter((item) => item.id !== restored.id),
              ],
              completedTasks: current.completedTasks.filter(
                (item) => item.id !== restored.id,
              ),
            }
          : current,
      );
      setProjectTasks((current) =>
        current.map((item) => (item.id === restored.id ? restored : item)),
      );
      if (task.projectId) {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? { ...project, openTaskCount: project.openTaskCount + 1 }
              : project,
          ),
        );
      }
      void loadHomeSnapshot();
      void loadGoals();
    } catch {
      setPlanningError(copy.messages.taskRestoreNotice);
      void loadPlanningSnapshot();
      if (selectedProjectId) void loadProjectTasks(selectedProjectId);
    }
  }

  async function createPlanningTask(input: {
    title: string;
    notes?: string;
    priority: number;
    dueAt?: string;
  }): Promise<void> {
    setPlanningError(undefined);
    try {
      const created = await withAuthenticatedSession((accessToken) =>
        createTask(apiBaseUrl, accessToken, input),
      );
      setHighlightedScheduleId(undefined);
      setHighlightedPlanningTaskId(created.id);
      await Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
    } catch (error) {
      setPlanningError(copy.messages.taskCreateNotice);
      throw error;
    }
  }

  async function createPlanningSchedule(input: {
    title: string;
    notes?: string;
    startsAt: string;
    endsAt: string;
  }): Promise<void> {
    setPlanningError(undefined);
    const clientMutationId = createUuidV7();
    try {
      const created = await withAuthenticatedSession((accessToken) =>
        createScheduleEntry(apiBaseUrl, accessToken, {
          ...input,
          clientMutationId,
        }),
      );
      setHighlightedPlanningTaskId(undefined);
      setHighlightedScheduleId(created.id);
      await Promise.all([
        loadHomeSnapshot(),
        loadPlanningSnapshot(created.startsAt),
      ]);
    } catch (error) {
      setPlanningError(copy.messages.scheduleCreateNotice);
      throw error;
    }
  }

  async function savePlanningTask(
    task: Task,
    input: {
      title: string;
      notes?: string;
      status: Task["status"];
      priority: number;
      dueAt?: string;
    },
  ): Promise<void> {
    setPlanningError(undefined);
    const updated = await withAuthenticatedSession((accessToken) =>
      updateTask(apiBaseUrl, accessToken, task, input),
    );
    setPlanningSnapshot((current) =>
      current
        ? {
            ...current,
            tasks: current.tasks.map((item) =>
              item.id === updated.id ? updated : item,
            ),
          }
        : current,
    );
    setProjectTasks((current) =>
      current.map((item) => (item.id === updated.id ? updated : item)),
    );
    await Promise.all([
      loadHomeSnapshot(),
      loadPlanningSnapshot(),
      loadGoals(),
      task.projectId && task.projectId === selectedProjectId
        ? loadProjectTasks(task.projectId)
        : Promise.resolve(undefined),
    ]);
  }

  async function deletePlanningTask(task: Task): Promise<void> {
    setPlanningError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        deleteTask(apiBaseUrl, accessToken, task),
      );
      await cancelLocalReminder("task", task.id).catch(() => false);
      setHomeSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
            }
          : current,
      );
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
              completedTasks: current.completedTasks.filter(
                (item) => item.id !== task.id,
              ),
            }
          : current,
      );
      setProjectTasks((current) =>
        current.filter((item) => item.id !== task.id),
      );
      if (task.projectId && task.status === "open") {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? {
                  ...project,
                  openTaskCount: Math.max(0, project.openTaskCount - 1),
                }
              : project,
          ),
        );
      }
      await Promise.all([
        loadHomeSnapshot(),
        loadPlanningSnapshot(),
        loadGoals(),
        task.projectId && task.projectId === selectedProjectId
          ? loadProjectTasks(task.projectId)
          : Promise.resolve(undefined),
      ]);
    } catch (error) {
      setPlanningError(copy.messages.taskDeleteNotice);
      throw error;
    }
  }

  async function savePlanningSchedule(
    entry: ScheduleEntry,
    input: {
      title: string;
      notes?: string;
      startsAt: string;
      endsAt: string;
    },
  ): Promise<void> {
    if (!entry.editable) throw new Error("schedule is read only");
    setPlanningError(undefined);
    const updated = await withAuthenticatedSession((accessToken) =>
      updateScheduleEntry(apiBaseUrl, accessToken, entry, input),
    );
    setPlanningSnapshot((current) =>
      current
        ? {
            ...current,
            schedule: current.schedule.map((item) =>
              item.id === updated.id ? updated : item,
            ),
          }
        : current,
    );
    await Promise.all([
      loadHomeSnapshot(),
      loadPlanningSnapshot(updated.startsAt),
    ]);
  }

  async function deletePlanningSchedule(entry: ScheduleEntry): Promise<void> {
    if (!entry.editable) throw new Error("schedule is read only");
    setPlanningError(undefined);
    await withAuthenticatedSession((accessToken) =>
      deleteScheduleEntry(apiBaseUrl, accessToken, entry),
    );
    await cancelLocalReminder("schedule", entry.id).catch(() => false);
    setPlanningSnapshot((current) =>
      current
        ? {
            ...current,
            schedule: current.schedule.filter((item) => item.id !== entry.id),
          }
        : current,
    );
    await Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
  }

  function selectWorkspace(workspaceId: string) {
    if (workspaceId === selectedWorkspaceId) return;
    setHighlightedProjectTaskId(undefined);
    setSelectedWorkspaceId(workspaceId);
    setSelectedProjectId(undefined);
    setProjectTasks([]);
  }

  function selectProject(projectId: string) {
    setHighlightedProjectTaskId(undefined);
    setSelectedProjectId(projectId);
  }

  async function openProjectFromAssistant(
    project: Pick<Project, "id" | "workspaceId">,
  ): Promise<void> {
    const loaded = await loadProjectsForWorkspace(
      project.workspaceId,
      project.id,
    );
    if (!loaded) throw new Error("project destination unavailable");
    if (!(await loadProjectTasks(project.id))) {
      throw new Error("project destination unavailable");
    }
    setHighlightedProjectTaskId(undefined);
    setSelectedWorkspaceId(project.workspaceId);
    setSelectedProjectId(project.id);
    setDestination("projects");
  }

  async function openTaskFromAssistant(
    task: Pick<Task, "id" | "projectId">,
  ): Promise<void> {
    if (!task.projectId) {
      const snapshot = await loadHomeSnapshot();
      if (!snapshot?.tasks.some((item) => item.id === task.id)) {
        throw new Error("task destination unavailable");
      }
      return;
    }
    const currentProject = projects.find(
      (project) => project.id === task.projectId,
    );
    if (currentProject) {
      const loaded = await loadProjectsForWorkspace(
        currentProject.workspaceId,
        currentProject.id,
      );
      if (!loaded) throw new Error("task destination unavailable");
      const tasks = await loadProjectTasks(currentProject.id);
      if (!tasks?.some((item) => item.id === task.id)) {
        throw new Error("task destination unavailable");
      }
      setHighlightedProjectTaskId(task.id);
      setSelectedProjectId(currentProject.id);
      setDestination("projects");
      return;
    }

    for (const workspace of workspaces) {
      try {
        const workspaceProjects = await withAuthenticatedSession(
          (accessToken) => fetchProjects(apiBaseUrl, accessToken, workspace.id),
        );
        const project = workspaceProjects.find(
          (item) => item.id === task.projectId,
        );
        if (!project) continue;
        const tasks = await loadProjectTasks(project.id);
        if (!tasks?.some((item) => item.id === task.id)) continue;
        setProjects(workspaceProjects);
        setSelectedWorkspaceId(workspace.id);
        setSelectedProjectId(project.id);
        setHighlightedProjectTaskId(task.id);
        setDestination("projects");
        return;
      } catch {
        // Keep searching the remaining personal workspaces.
      }
    }

    setHomeError(copy.home.taskDestinationNotice);
    throw new Error("task destination unavailable");
  }

  async function openScheduleFromAssistant(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): Promise<void> {
    const snapshot = await loadPlanningSnapshot(entry.startsAt);
    if (!snapshot?.schedule.some((item) => item.id === entry.id)) {
      setHomeError(copy.home.scheduleDestinationNotice);
      setPlanningError(copy.home.scheduleDestinationNotice);
      return;
    }
    setHighlightedPlanningTaskId(undefined);
    setHighlightedScheduleId(entry.id);
    setDestination("calendar");
  }

  async function openPlanningTask(task: Task): Promise<void> {
    const snapshot = await loadPlanningSnapshot();
    if (!snapshot?.tasks.some((item) => item.id === task.id)) {
      setHomeError(copy.home.taskDestinationNotice);
      setPlanningError(copy.home.taskDestinationNotice);
      return;
    }
    setHighlightedScheduleId(undefined);
    setHighlightedPlanningTaskId(task.id);
    setDestination("calendar");
  }

  async function createWorkspaceProject(input: {
    title: string;
    objective?: string;
    riskLevel: number;
    nextAction?: string;
    dueAt?: string;
  }): Promise<void> {
    if (!selectedWorkspaceId) throw new Error("workspace unavailable");
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const project = await withAuthenticatedSession((accessToken) =>
        createProject(apiBaseUrl, accessToken, {
          workspaceId: selectedWorkspaceId,
          ...input,
        }),
      );
      await loadProjectsForWorkspace(selectedWorkspaceId, project.id);
    } catch (error) {
      setProjectsError(copy.messages.projectSaveNotice);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function createWorkspaceGoal(input: {
    title: string;
    desiredOutcome: string;
    projectId?: string;
    targetAt?: string;
  }): Promise<void> {
    if (!selectedWorkspaceId) throw new Error("workspace unavailable");
    setGoalsSaving(true);
    setGoalsError(undefined);
    try {
      const goal = await withAuthenticatedSession((accessToken) =>
        createGoal(apiBaseUrl, accessToken, {
          workspaceId: selectedWorkspaceId,
          ...input,
        }),
      );
      setGoals((current) => [goal, ...current]);
      void loadHomeSnapshot();
    } catch (error) {
      setGoalsError(copy.goals.saveProblem);
      throw error;
    } finally {
      setGoalsSaving(false);
    }
  }

  async function updateWorkspaceGoal(
    goal: Goal,
    input: {
      title: string;
      desiredOutcome: string;
      status: Goal["status"];
      projectId?: string;
      targetAt?: string;
    },
  ): Promise<void> {
    setGoalsSaving(true);
    setGoalsError(undefined);
    try {
      const updated = await withAuthenticatedSession((accessToken) =>
        updateGoal(apiBaseUrl, accessToken, goal, {
          workspaceId: goal.workspaceId ?? selectedWorkspaceId,
          ...input,
        }),
      );
      setGoals((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      void loadHomeSnapshot();
    } catch (error) {
      setGoalsError(copy.goals.saveProblem);
      void loadGoals();
      throw error;
    } finally {
      setGoalsSaving(false);
    }
  }

  async function updateWorkspaceProject(
    project: Project,
    input: {
      title: string;
      objective?: string;
      status: Project["status"];
      riskLevel: number;
      nextAction?: string;
      dueAt?: string;
    },
  ): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const updated = await withAuthenticatedSession((accessToken) =>
        updateProject(apiBaseUrl, accessToken, project, input),
      );
      setProjects((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
      void loadGoals();
    } catch (error) {
      setProjectsError(copy.projects.projectUpdateNotice);
      if (selectedWorkspaceId) {
        void loadProjectsForWorkspace(selectedWorkspaceId, project.id);
      }
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function deleteWorkspaceProject(project: Project): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        deleteProject(apiBaseUrl, accessToken, project),
      );
      setProjects((current) =>
        current.filter((item) => item.id !== project.id),
      );
      setSelectedProjectId(undefined);
      setHighlightedProjectTaskId(undefined);
      setProjectTasks([]);
      setProjectWebhooks([]);
      setWebhookDeliveries([]);
      await Promise.all([
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId)
          : Promise.resolve(false),
        loadHomeSnapshot(),
        loadPlanningSnapshot(),
        loadGoals(),
      ]);
    } catch (error) {
      setProjectsError(copy.projects.projectDeleteNotice);
      if (selectedWorkspaceId) {
        void loadProjectsForWorkspace(selectedWorkspaceId, project.id);
      }
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function createProjectTask(title: string): Promise<void> {
    if (!selectedProjectId) throw new Error("project unavailable");
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const task = await withAuthenticatedSession((accessToken) =>
        createTask(apiBaseUrl, accessToken, {
          title,
          priority: 1,
          projectId: selectedProjectId,
        }),
      );
      setProjectTasks((current) => [...current, task]);
      setProjects((current) =>
        current.map((project) =>
          project.id === selectedProjectId
            ? { ...project, openTaskCount: project.openTaskCount + 1 }
            : project,
        ),
      );
      void loadHomeSnapshot();
      void loadGoals();
    } catch (error) {
      setProjectsError(copy.messages.projectTaskSaveNotice);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function completeProjectTask(task: Task): Promise<void> {
    if (!tokens) return;
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const completed = await withAuthenticatedSession((accessToken) =>
        completeTask(apiBaseUrl, accessToken, task),
      );
      await cancelLocalReminder("task", task.id).catch(() => false);
      setProjectTasks((current) =>
        current.map((item) => (item.id === completed.id ? completed : item)),
      );
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== completed.id),
              completedTasks: [
                completed,
                ...current.completedTasks.filter(
                  (item) => item.id !== completed.id,
                ),
              ],
            }
          : current,
      );
      if (task.projectId) {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? {
                  ...project,
                  openTaskCount: Math.max(0, project.openTaskCount - 1),
                }
              : project,
          ),
        );
      }
      void loadHomeSnapshot();
      void loadGoals();
    } catch {
      setProjectsError(copy.messages.taskCompletionNotice);
      if (selectedProjectId) void loadProjectTasks(selectedProjectId);
    } finally {
      setProjectsSaving(false);
    }
  }

  async function updateProjectTask(
    task: Task,
    input: {
      title: string;
      notes?: string;
      status: Task["status"];
      priority: number;
      dueAt?: string;
    },
  ): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const updated = await withAuthenticatedSession((accessToken) =>
        updateTask(apiBaseUrl, accessToken, task, input),
      );
      setProjectTasks((current) =>
        updated.status === "cancelled"
          ? current.filter((item) => item.id !== updated.id)
          : current.map((item) => (item.id === updated.id ? updated : item)),
      );
      const openDelta =
        Number(updated.status === "open") - Number(task.status === "open");
      if (openDelta && task.projectId) {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? {
                  ...project,
                  openTaskCount: Math.max(0, project.openTaskCount + openDelta),
                }
              : project,
          ),
        );
      }
      void loadHomeSnapshot();
      void loadPlanningSnapshot();
      void loadGoals();
    } catch {
      setProjectsError(copy.messages.projectTaskSaveNotice);
      if (selectedProjectId) void loadProjectTasks(selectedProjectId);
      throw new Error("task update failed");
    } finally {
      setProjectsSaving(false);
    }
  }

  async function deleteProjectTask(task: Task): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        deleteTask(apiBaseUrl, accessToken, task),
      );
      await cancelLocalReminder("task", task.id).catch(() => false);
      setProjectTasks((current) =>
        current.filter((item) => item.id !== task.id),
      );
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: current.tasks.filter((item) => item.id !== task.id),
              completedTasks: current.completedTasks.filter(
                (item) => item.id !== task.id,
              ),
            }
          : current,
      );
      if (task.status === "open" && task.projectId) {
        setProjects((current) =>
          current.map((project) =>
            project.id === task.projectId
              ? {
                  ...project,
                  openTaskCount: Math.max(0, project.openTaskCount - 1),
                }
              : project,
          ),
        );
      }
      await Promise.all([
        loadHomeSnapshot(),
        loadPlanningSnapshot(),
        loadGoals(),
      ]);
    } catch (error) {
      setProjectsError(copy.projects.taskRemoveNotice);
      if (selectedProjectId) void loadProjectTasks(selectedProjectId);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function createWorkspaceWebhook(input: {
    provider: ManagedWebhookProvider;
    url: string;
    events: ProjectWebhookEvent[];
  }): Promise<void> {
    if (!selectedProjectId) throw new Error("project unavailable");
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const webhook = await withAuthenticatedSession((accessToken) =>
        createProjectWebhook(apiBaseUrl, accessToken, selectedProjectId, input),
      );
      setProjectWebhooks((current) => [...current, webhook]);
    } catch (error) {
      setProjectsError(copy.projects.webhookSaveProblem);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function updateWorkspaceWebhook(
    webhook: ProjectWebhook,
    input: {
      provider: ManagedWebhookProvider;
      destinationMode: WebhookDestinationMode;
      url?: string;
      events: ProjectWebhookEvent[];
      enabled: boolean;
    },
  ): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const updated = await withAuthenticatedSession((accessToken) =>
        updateProjectWebhook(apiBaseUrl, accessToken, webhook, input),
      );
      setProjectWebhooks((current) =>
        current.map((item) => (item.id === updated.id ? updated : item)),
      );
    } catch (error) {
      setProjectsError(copy.projects.webhookUpdateProblem);
      void loadProjectWebhooks(webhook.projectId);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function testWorkspaceWebhook(webhook: ProjectWebhook): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        testProjectWebhook(apiBaseUrl, accessToken, webhook),
      );
      for (let attempt = 0; attempt < 8; attempt += 1) {
        const snapshot = await loadProjectWebhooks(webhook.projectId);
        const latestTest = snapshot?.deliveries.find(
          (delivery) =>
            delivery.webhookId === webhook.id &&
            delivery.eventType === "webhook.test",
        );
        if (
          latestTest?.status === "delivered" ||
          latestTest?.status === "failed"
        ) {
          break;
        }
        if (attempt < 7) {
          await new Promise<void>((resolve) => {
            const timeoutId = window.setTimeout(() => {
              window.clearTimeout(timeoutId);
              resolve();
            }, 400);
          });
        }
      }
    } catch (error) {
      setProjectsError(copy.projects.webhookTestProblem);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function retryWorkspaceWebhookDelivery(
    delivery: WebhookDelivery,
  ): Promise<void> {
    if (!selectedProjectId) throw new Error("project unavailable");
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        retryWebhookDelivery(
          apiBaseUrl,
          accessToken,
          selectedProjectId,
          delivery.id,
        ),
      );
      await loadProjectWebhooks(selectedProjectId);
    } catch (error) {
      setProjectsError(copy.projects.webhookRetryProblem);
      void loadProjectWebhooks(selectedProjectId);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  async function deleteWorkspaceWebhook(
    webhook: ProjectWebhook,
  ): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        deleteProjectWebhook(apiBaseUrl, accessToken, webhook),
      );
      setProjectWebhooks((current) =>
        current.filter((item) => item.id !== webhook.id),
      );
    } catch (error) {
      setProjectsError(copy.projects.webhookDeleteProblem);
      throw error;
    } finally {
      setProjectsSaving(false);
    }
  }

  function openNewAssistantRequest() {
    startConversation();
    setAssistantDraft(undefined);
    setDestination("chat");
  }

  function handleVoiceTranscript(value: string) {
    startConversation();
    setAssistantDraft({ id: createUuidV7(), text: value, autoSend: true });
    setDestination("chat");
  }

  async function handleVoiceCommand(
    value: string,
  ): Promise<VoiceCommandOutcome> {
    if (!tokens) {
      return {
        kind: "failed",
        message: copy.voice.commandFailed,
      };
    }
    const clientMutationId = createUuidV7();
    try {
      const result = await withAuthenticatedSession((accessToken) =>
        processVoiceCommand(apiBaseUrl, accessToken, value, clientMutationId),
      );
      if (result.kind === "schedule_listed" || result.kind === "tasks_listed") {
        return {
          kind: "query",
          message: result.message,
          destination: result.destination === "calendar" ? "calendar" : "home",
          items: result.items,
        };
      }
      if (
        result.kind === "schedule_created" ||
        result.kind === "task_created"
      ) {
        await Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
        return {
          kind: "handled",
          message: result.message,
          destination: result.destination === "calendar" ? "calendar" : "home",
          items: result.items,
        };
      }
      if (result.kind === "needs_details") {
        return { kind: "needs-details", message: result.message };
      }
      return { kind: "conversation", message: result.message };
    } catch {
      return {
        kind: "failed",
        message: copy.voice.commandFailed,
      };
    }
  }

  async function beginAgentAuthentication(): Promise<void> {
    if (!tokens || authenticationRequesting) return;
    setAuthenticationRequesting(true);
    openedAuthenticationUrl.current = undefined;
    setConversationError(undefined);
    try {
      setAgentAuthentication(
        await withAuthenticatedSession((accessToken) =>
          requestAgentAuthentication(apiBaseUrl, accessToken),
        ),
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
    options: ConversationSendOptions = {},
  ): Promise<boolean> {
    if (!tokens || agentAuthentication?.state !== "ready") {
      setConversationError(copy.messages.authenticationRequired);
      return false;
    }
    let conversationId = conversationIdForRequest(
      selectedConversationId,
      options,
    );
    setConversationError(undefined);
    try {
      if (!conversationId) {
        const clientConversationId =
          pendingConversationId.current ?? createUuidV7();
        pendingConversationId.current = clientConversationId;
        const conversation = await withAuthenticatedSession((accessToken) =>
          createConversation(
            apiBaseUrl,
            accessToken,
            clientConversationId,
            conversationTitle(text),
          ),
        );
        pendingConversationId.current = undefined;
        conversationId = conversation.id;
        setConversations((current) => [conversation, ...current]);
        setSelectedConversationId(conversation.id);
      }
      if (!conversationId) {
        setConversationError(copy.messages.conversationSendNotice);
        return false;
      }
      const targetConversationId = conversationId;
      if (selectedConversationId !== targetConversationId) {
        setSelectedConversationId(targetConversationId);
        setConversationMessages([]);
      }
      if (options.rememberForHome) {
        setHomeConversationId(targetConversationId);
      }
      const queued = await withAuthenticatedSession((accessToken) =>
        queueAgentTurn(
          apiBaseUrl,
          accessToken,
          targetConversationId,
          text.trim(),
          clientMessageId,
        ),
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
          pendingAction: null,
        },
      }));
      await loadConversationMessages(queued.conversationId);
      void refreshConversations();
      if (isTerminalAgentJob(queued.state)) {
        await loadHomeSnapshot();
      }
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

  async function resolveConversationAction(
    decision: "approve" | "decline",
  ): Promise<void> {
    if (!tokens || !selectedConversationId) return;
    const job = conversationJobs[selectedConversationId];
    if (!job || job.state !== "waiting_approval") return;
    setConversationLoading(true);
    setConversationError(undefined);
    try {
      const resolved = await withAuthenticatedSession((accessToken) =>
        resolveAgentAction(apiBaseUrl, accessToken, job.id, decision),
      );
      setConversationJobs((known) => ({
        ...known,
        [resolved.conversationId]: resolved,
      }));
      await Promise.all([
        loadConversationMessages(resolved.conversationId, true),
        loadHomeSnapshot(),
        refreshConversations(),
      ]);
    } catch {
      setConversationError(copy.messages.actionResolutionNotice);
    } finally {
      setConversationLoading(false);
    }
  }

  const showLaunchSplash =
    !sessionLoaded ||
    (mode === "loading" &&
      homeSnapshot === undefined &&
      agentAuthentication === undefined &&
      conversations.length === 0);
  const latestAssistantMessage =
    assistantResponseAfterLatestRequest(conversationMessages);
  const latestUserRequest = [...conversationMessages]
    .reverse()
    .find((message) => message.role === "user")?.content;

  function navigate(nextDestination: OsDestination): void {
    setDestination(nextDestination);
    if (
      nextDestination === "home" &&
      homeConversationId &&
      selectedConversationId !== homeConversationId
    ) {
      setSelectedConversationId(homeConversationId);
      setConversationMessages([]);
      void loadConversationMessages(homeConversationId);
      void restoreConversationJob(homeConversationId);
      return;
    }
    if (nextDestination === "calendar") {
      const latestSchedule = [
        ...(latestAssistantMessage?.presentation?.items ?? []),
      ]
        .reverse()
        .find((item) => item.type === "schedule");
      void loadPlanningSnapshot(latestSchedule?.startsAt);
      return;
    }
    if (nextDestination === "projects") {
      void loadGoals();
      const latestProject = [
        ...(latestAssistantMessage?.presentation?.items ?? []),
      ]
        .reverse()
        .find((item) => item.type === "project");
      if (latestProject) {
        setSelectedWorkspaceId(latestProject.workspaceId);
        void loadProjectsForWorkspace(
          latestProject.workspaceId,
          latestProject.id,
        );
      } else if (selectedWorkspaceId) {
        void loadProjectsForWorkspace(selectedWorkspaceId);
      }
    }
    if (nextDestination === "decisions") {
      void loadDecisionInbox();
    }
  }

  return (
    <div
      className="app-shell"
      data-app-state={showLaunchSplash ? "launching" : "active"}
    >
      {showLaunchSplash ? (
        <LaunchSplash />
      ) : mode === "configuration" ? (
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
          onNavigate={navigate}
          onVoiceTranscript={handleVoiceTranscript}
          onVoiceCommand={handleVoiceCommand}
          onRefresh={() =>
            void (destination === "decisions" ? loadDecisionInbox() : refresh())
          }
          refreshing={
            mode === "loading" ||
            (destination === "decisions" && decisionsLoading)
          }
          rail={
            destination !== "chat" ? (
              <AssistantRail
                assistantReady={agentAuthentication?.state === "ready"}
                onOpenAssistant={openNewAssistantRequest}
              />
            ) : undefined
          }
        >
          {destination === "home" && (
            <HomeWorkspace
              snapshot={homeSnapshot}
              loading={homeLoading || mode === "loading"}
              error={homeError ?? (mode === "error" ? message : undefined)}
              assistantReady={agentAuthentication?.state === "ready"}
              assistantJob={
                homeConversationId
                  ? conversationJobs[homeConversationId]
                  : undefined
              }
              assistantConversationId={homeConversationId}
              assistantRequest={
                selectedConversationId === homeConversationId
                  ? latestUserRequest
                  : undefined
              }
              assistantMessage={
                selectedConversationId === homeConversationId
                  ? latestAssistantMessage
                  : undefined
              }
              onOpenAssistant={openHomeAssistant}
              onStartNewAssistant={startHomeConversation}
              onSendAssistant={(text, clientMessageId) =>
                sendConversationRequest(text, clientMessageId, {
                  startFresh: !homeConversationId,
                  targetConversationId: homeConversationId,
                  rememberForHome: true,
                })
              }
              onCompleteTask={completeHomeTask}
              onEditTask={(task) =>
                setPlanningEditTarget({ kind: "task", item: task })
              }
              onEditSchedule={(entry) =>
                setPlanningEditTarget({ kind: "schedule", item: entry })
              }
              onOpenPlanningTask={openPlanningTask}
              onOpenTask={openTaskFromAssistant}
              onOpenProject={openProjectFromAssistant}
              onOpenSchedule={openScheduleFromAssistant}
              onOpenDecisionInbox={() => navigate("decisions")}
              onDecideRecommendation={decideHomeRecommendation}
            />
          )}
          {destination === "calendar" && (
            <PlanningWorkspace
              snapshot={planningSnapshot}
              range={planningRange}
              calendarConnection={calendarConnection}
              loading={planningLoading || mode === "loading"}
              error={planningError ?? (mode === "error" ? message : undefined)}
              highlightedScheduleId={highlightedScheduleId}
              highlightedTaskId={highlightedPlanningTaskId}
              onCompleteTask={completeHomeTask}
              onRestoreTask={restorePlanningTask}
              onCreateTask={createPlanningTask}
              onCreateSchedule={createPlanningSchedule}
              onEditTask={(task) =>
                setPlanningEditTarget({ kind: "task", item: task })
              }
              onEditSchedule={(entry) =>
                setPlanningEditTarget({ kind: "schedule", item: entry })
              }
              onRangeChange={changePlanningRange}
              onSyncCalendar={syncGoogleCalendar}
            />
          )}
          {destination === "projects" && (
            <ProjectsWorkspace
              workspaces={workspaces}
              goals={goals}
              projects={projects}
              tasks={projectTasks}
              webhooks={projectWebhooks}
              webhookDeliveries={webhookDeliveries}
              selectedWorkspaceId={selectedWorkspaceId}
              selectedProjectId={selectedProjectId}
              highlightedTaskId={highlightedProjectTaskId}
              loading={projectsLoading || goalsLoading || mode === "loading"}
              webhookLoading={webhooksLoading}
              saving={projectsSaving || goalsSaving}
              error={goalsError ?? projectsError}
              onSelectWorkspace={selectWorkspace}
              onSelectProject={selectProject}
              onOpenGoalTask={(taskId, projectId) =>
                void openTaskFromAssistant({ id: taskId, projectId })
              }
              onClearProject={() => {
                setHighlightedProjectTaskId(undefined);
                setSelectedProjectId(undefined);
                setProjectTasks([]);
                setProjectWebhooks([]);
                setWebhookDeliveries([]);
              }}
              onCreateProject={createWorkspaceProject}
              onCreateGoal={createWorkspaceGoal}
              onUpdateGoal={updateWorkspaceGoal}
              onUpdateProject={updateWorkspaceProject}
              onDeleteProject={deleteWorkspaceProject}
              onCreateTask={createProjectTask}
              onCompleteTask={completeProjectTask}
              onUpdateTask={updateProjectTask}
              onDeleteTask={deleteProjectTask}
              onCreateWebhook={createWorkspaceWebhook}
              onUpdateWebhook={updateWorkspaceWebhook}
              onTestWebhook={testWorkspaceWebhook}
              onDeleteWebhook={deleteWorkspaceWebhook}
              onRetryWebhookDelivery={retryWorkspaceWebhookDelivery}
            />
          )}
          {destination === "decisions" && (
            <DecisionInboxWorkspace
              recommendations={decisionRecommendations}
              loading={decisionsLoading || mode === "loading"}
              error={decisionsError}
              onDecide={decideHomeRecommendation}
            />
          )}
          {destination === "memory" && (
            <MemoryWorkspace onOpenConversation={openNewAssistantRequest} />
          )}
          {destination === "settings" && (
            <SettingsWorkspace
              authentication={agentAuthentication}
              requesting={authenticationRequesting}
              modelSettings={agentModelSettings}
              modelsLoading={agentModelsLoading}
              modelsSaving={agentModelsSaving}
              modelsError={agentModelsError}
              calendarConnection={calendarConnection}
              calendarLoading={calendarLoading}
              calendarAction={calendarAction}
              calendarAuthorizationPending={Boolean(
                calendarAuthorizationExpiresAt,
              )}
              calendarError={calendarError}
              reminderSyncStatus={reminderSyncStatus}
              reminderSyncError={reminderSyncError}
              remoteReminderStatus={remoteReminderStatus}
              onStartAuthentication={beginAgentAuthentication}
              onReloadModels={loadAgentModelSettings}
              onSaveModel={saveAgentModelSettings}
              onStartCalendarConnection={beginGoogleCalendarConnection}
              onReloadCalendarConnection={loadGoogleCalendarConnection}
              onSyncCalendar={syncGoogleCalendar}
              onDisconnectCalendar={disconnectGoogleCalendarConnection}
              onRetryReminderSync={synchronizePlanningReminders}
            />
          )}
          {destination === "chat" && (
            <ConversationWorkspace
              conversations={conversations}
              messages={conversationMessages}
              selectedConversationId={selectedConversationId}
              job={
                selectedConversationId
                  ? conversationJobs[selectedConversationId]
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
              initialDraft={assistantDraft}
              onSelect={selectConversation}
              onInitialDraftApplied={() => setAssistantDraft(undefined)}
              onStartConversation={startConversation}
              onStartAuthentication={beginAgentAuthentication}
              onSend={sendConversationRequest}
              onResolveAction={resolveConversationAction}
            />
          )}
          <PlanningItemEditor
            target={planningEditTarget}
            onClose={() => setPlanningEditTarget(undefined)}
            onSaveTask={savePlanningTask}
            onSaveSchedule={savePlanningSchedule}
            onDeleteTask={deletePlanningTask}
            onDeleteSchedule={deletePlanningSchedule}
          />
        </OsShell>
      )}
    </div>
  );
}

function LaunchSplash() {
  return (
    <main className="launch-splash" aria-busy="true">
      <div className="launch-splash__content">
        <span className="launch-splash__mark" aria-hidden="true">
          <Sparkles />
        </span>
        <div className="launch-splash__copy">
          <strong>{copy.productName}</strong>
          <p role="status" aria-live="polite">
            {copy.launch.loading}
          </p>
        </div>
        <div className="launch-splash__progress" aria-hidden="true">
          <span />
        </div>
      </div>
    </main>
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

function currentReminderRange(now = new Date()): [Date, Date] {
  const from = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const to = new Date(from.getFullYear(), from.getMonth(), from.getDate() + 91);
  return [from, to];
}

async function openExternalUrl(url: string): Promise<void> {
  try {
    await openUrl(url);
  } catch {
    const opened = window.open(url, "_blank", "noopener,noreferrer");
    if (!opened) throw new Error("external navigation unavailable");
  }
}

function isTerminalAgentJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}
