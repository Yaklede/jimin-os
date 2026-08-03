import { Server, Sparkles } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  lazy,
  Suspense,
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
  disconnectGmailAccount,
  fetchGmailAccounts,
  gmailAuthorizationBaseline,
  gmailAuthorizationChanged,
  startGmailAuthorization,
  synchronizeGmailAccount,
  type GmailAccount,
  type GmailAuthorizationBaseline,
} from "./api/gmail";
import {
  decideGmailInflow,
  emptyGmailInflowLoadHealth,
  fetchGmailInflow,
  gmailInflowHealthAfterInitial,
  gmailInflowHealthAfterLoadMore,
  type GmailInflowCandidate,
  type GmailInflowLoadHealth,
} from "./api/gmailInflow";
import {
  bootstrapTrustedNetworkSession,
  completeTask,
  createScheduleEntry,
  createTask,
  deleteTask,
  deleteScheduleEntry,
  fetchTask,
  refreshDeviceSession,
  updateTask,
  updateScheduleEntry,
  fetchPlanning,
  PlanningRequestError,
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
  fetchWeeklyReport,
  fetchWeeklyReportHistory,
  fetchWorkspaces,
  updateProject,
  type Project,
  type WeeklyReport,
  type WeeklyReportSnapshot,
  type Workspace,
} from "./api/projects";
import {
  createProjectWeeklyReport as createProjectWeeklyReportRequest,
  finalizeReport as finalizeReportRequest,
  fetchProjectReports,
  updateReport as updateReportRequest,
  type ProjectWeeklyReportContent,
  type Report,
} from "./api/reports";
import { createGoal, fetchGoals, updateGoal, type Goal } from "./api/goals";
import {
  createProjectGoogleChatSource,
  decideProjectInflow,
  deleteProjectGoogleChatSource,
  fetchGoogleChatConnections,
  fetchGoogleChatSpaces,
  fetchProjectGoogleChatSources,
  fetchProjectInflow,
  GoogleChatRequestError,
  startGoogleChatAuthorization,
  syncProjectGoogleChatSource,
  type GoogleChatAccount,
  type GoogleChatSpace,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "./api/googleChat";
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
  type WebhookMentionDirectory,
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
  fetchDeviceSignalStates,
  synchronizeMissedCalls,
  type DeviceSignalState,
} from "./api/deviceSignals";
import {
  fetchSyncChanges,
  streamSyncCursor,
  type SyncChange,
} from "./api/sync";
import { LatestRequestGate } from "./latestRequestGate";
import { SyncPullCoordinator } from "./syncPullCoordinator";
import {
  AgentRequestError,
  archiveConversation,
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
import { assistantResponseAfterLatestRequest } from "./components/conversationResponse";
import { HomeWorkspace } from "./components/HomeWorkspace";
import { type PromoteGmailInflowInput } from "./components/GmailInflowReview";
import { OsShell, type OsDestination } from "./components/OsShell";
import {
  inflowConversationKey,
  type PromoteInflowInput,
} from "./components/ProjectInflowPanel";
import { type PlanningEditTarget } from "./components/PlanningItemEditor";
import { type VoiceCommandOutcome } from "./components/VoiceCommandSheet";
import { copy } from "./copy";
import {
  deviceSignalsSupported,
  getCallLogPermission,
  readNativeMissedCalls,
  requestCallLogPermission,
  type NativeCallLogPermission,
} from "./device-signals";
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
import {
  installAndroidBackBridge,
  registerMobileBackHandler,
} from "./mobileBack";
import {
  calendarDestinationActivation,
  calendarDestinationLoad,
  type CalendarNavigationIntent,
} from "./calendarNavigation";
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
import { WorkspaceRouteBoundary } from "./components/WorkspaceRouteBoundary";

type AppMode =
  "configuration" | "server-unreachable" | "loading" | "ready" | "error";
type ConversationJobs = Record<string, AgentJob>;
type AssistantDraft = {
  id: string;
  text: string;
  autoSend: boolean;
};
type GmailAction =
  | { kind: "authorizing"; workspaceId: string; accountId?: string }
  | { kind: "syncing" | "disconnecting"; accountId: string };

const ConversationWorkspace = lazy(() =>
  import("./components/ConversationWorkspace").then((module) => ({
    default: module.ConversationWorkspace,
  })),
);
const DecisionInboxWorkspace = lazy(() =>
  import("./components/DecisionInboxWorkspace").then((module) => ({
    default: module.DecisionInboxWorkspace,
  })),
);
const MemoryWorkspace = lazy(() =>
  import("./components/MemoryWorkspace").then((module) => ({
    default: module.MemoryWorkspace,
  })),
);
const MeetingsWorkspace = lazy(() =>
  import("./components/MeetingsWorkspace").then((module) => ({
    default: module.MeetingsWorkspace,
  })),
);
const PlanningWorkspace = lazy(() =>
  import("./components/PlanningWorkspace").then((module) => ({
    default: module.PlanningWorkspace,
  })),
);
const PlanningItemEditor = lazy(() =>
  import("./components/PlanningItemEditor").then((module) => ({
    default: module.PlanningItemEditor,
  })),
);
const ProjectsWorkspace = lazy(() =>
  import("./components/ProjectsWorkspace").then((module) => ({
    default: module.ProjectsWorkspace,
  })),
);
const SettingsWorkspace = lazy(() =>
  import("./components/SettingsWorkspace").then((module) => ({
    default: module.SettingsWorkspace,
  })),
);

export default function App() {
  const apiBaseUrl = personalServerBaseUrl ?? "";
  const [tokens, setTokens] = useState<SessionTokens | undefined>(undefined);
  const [sessionLoaded, setSessionLoaded] = useState(false);
  const [mode, setMode] = useState<AppMode>("loading");
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [destination, setDestination] = useState<OsDestination>("home");
  const navigationHistoryRef = useRef<OsDestination[]>([]);
  const calendarNavigationIntentRef = useRef<
    CalendarNavigationIntent | undefined
  >(undefined);
  const calendarDestinationActiveRef = useRef(false);
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
  const [weeklyReport, setWeeklyReport] = useState<WeeklyReport>();
  const [weeklyReportHistory, setWeeklyReportHistory] = useState<
    WeeklyReportSnapshot[]
  >([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [projectTasks, setProjectTasks] = useState<Task[]>([]);
  const [projectReports, setProjectReports] = useState<Report[]>([]);
  const [projectWebhooks, setProjectWebhooks] = useState<ProjectWebhook[]>([]);
  const [googleChatAccountsAvailable, setGoogleChatAccountsAvailable] =
    useState(false);
  const [googleChatAccounts, setGoogleChatAccounts] = useState<
    GoogleChatAccount[]
  >([]);
  const [googleChatSpaces, setGoogleChatSpaces] = useState<GoogleChatSpace[]>(
    [],
  );
  const [projectGoogleChatSources, setProjectGoogleChatSources] = useState<
    ProjectGoogleChatSource[]
  >([]);
  const [projectInflowItems, setProjectInflowItems] = useState<
    ProjectInflowItem[]
  >([]);
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
  const [inflowLoading, setInflowLoading] = useState(false);
  const [reportsLoading, setReportsLoading] = useState(false);
  const [reportsSaving, setReportsSaving] = useState(false);
  const [inflowSaving, setInflowSaving] = useState(false);
  const [inflowError, setInflowError] = useState<string>();
  const [
    googleChatAuthorizationExpiresAt,
    setGoogleChatAuthorizationExpiresAt,
  ] = useState<string>();
  const [projectsSaving, setProjectsSaving] = useState(false);
  const [goalsLoading, setGoalsLoading] = useState(false);
  const [goalsSaving, setGoalsSaving] = useState(false);
  const [goalsError, setGoalsError] = useState<string>();
  const [projectsError, setProjectsError] = useState<string>();
  const [reportsError, setReportsError] = useState<string>();
  const [weeklyReportError, setWeeklyReportError] = useState<string>();
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
  const [gmailAccountsAvailable, setGmailAccountsAvailable] = useState(true);
  const [gmailAccounts, setGmailAccounts] = useState<GmailAccount[]>([]);
  const [gmailLoading, setGmailLoading] = useState(false);
  const [gmailActions, setGmailActions] = useState<GmailAction[]>([]);
  const [gmailAuthorizationPending, setGmailAuthorizationPending] = useState<
    { expiresAt: string } & GmailAuthorizationBaseline
  >();
  const [gmailError, setGmailError] = useState<string>();
  const [gmailInflowItems, setGmailInflowItems] = useState<
    GmailInflowCandidate[]
  >([]);
  const [gmailInflowProjects, setGmailInflowProjects] = useState<Project[]>([]);
  const [gmailInflowLoading, setGmailInflowLoading] = useState(false);
  const [gmailInflowLoadingMore, setGmailInflowLoadingMore] = useState(false);
  const [gmailInflowLoadHealth, setGmailInflowLoadHealth] =
    useState<GmailInflowLoadHealth>(emptyGmailInflowLoadHealth);
  const [gmailInflowCursors, setGmailInflowCursors] = useState<
    Record<string, string | null>
  >({});
  const [gmailInflowError, setGmailInflowError] = useState<string>();
  const [gmailInflowSavingId, setGmailInflowSavingId] = useState<string>();
  const [reminderSyncStatus, setReminderSyncStatus] =
    useState<ReminderSyncStatus>("idle");
  const [reminderSyncError, setReminderSyncError] = useState<string>();
  const [remoteReminderStatus, setRemoteReminderStatus] =
    useState<RemoteReminderStatus>("idle");
  const [deviceSignalStates, setDeviceSignalStates] = useState<
    DeviceSignalState[]
  >([]);
  const [nativeCallLogPermission, setNativeCallLogPermission] =
    useState<NativeCallLogPermission>();
  const [deviceSignalsLoading, setDeviceSignalsLoading] = useState(false);
  const [deviceSignalsError, setDeviceSignalsError] = useState<string>();
  const pendingConversationId = useRef<string | undefined>(undefined);
  const homeConversationDetachedRef = useRef(false);
  const homeConversationStartingRef = useRef(false);
  const selectedConversationIdRef = useRef<string | undefined>(undefined);
  const conversationListRequestGateRef = useRef(new LatestRequestGate());
  const conversationMessageRequestGateRef = useRef(new LatestRequestGate());
  const gmailInflowRequestGateRef = useRef(new LatestRequestGate());
  const openedAuthenticationUrl = useRef<string | undefined>(undefined);
  const activeSessionRef = useRef<SessionTokens | undefined>(undefined);
  const refreshInFlightRef = useRef<Promise<SessionTokens> | undefined>(
    undefined,
  );
  const syncCursorRef = useRef("0");
  const syncPullCoordinatorRef = useRef(new SyncPullCoordinator());
  const reminderSyncInFlightRef = useRef<Promise<boolean> | undefined>(
    undefined,
  );
  const deviceSignalSyncInFlightRef = useRef<Promise<boolean> | undefined>(
    undefined,
  );
  const pendingReminderInFlightRef = useRef(false);
  const projectDataReadyOnNavigationRef = useRef(false);
  const [message, setMessage] = useState<string | undefined>(undefined);

  const setCurrentConversationId = useCallback(
    (conversationId: string | undefined) => {
      selectedConversationIdRef.current = conversationId;
      conversationMessageRequestGateRef.current.invalidate();
      setConversationLoading(false);
      setSelectedConversationId(conversationId);
    },
    [],
  );

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
    const requestGeneration = conversationListRequestGateRef.current.begin();
    setConversationLoading(true);
    setConversationError(undefined);
    try {
      const nextConversations = await withAuthenticatedSession((accessToken) =>
        fetchConversations(apiBaseUrl, accessToken),
      );
      if (conversationListRequestGateRef.current.isCurrent(requestGeneration)) {
        setConversations(nextConversations);
      }
    } catch {
      if (conversationListRequestGateRef.current.isCurrent(requestGeneration)) {
        setConversationError(copy.messages.conversationLoadNotice);
      }
    } finally {
      if (conversationListRequestGateRef.current.isCurrent(requestGeneration)) {
        setConversationLoading(false);
      }
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

  const synchronizeDeviceSignals = useCallback(
    async (askForPermission = false): Promise<boolean> => {
      if (!tokens) return false;
      if (deviceSignalSyncInFlightRef.current) {
        return deviceSignalSyncInFlightRef.current;
      }
      const operation = (async () => {
        setDeviceSignalsLoading(true);
        setDeviceSignalsError(undefined);
        try {
          if (deviceSignalsSupported()) {
            const permission = askForPermission
              ? await requestCallLogPermission()
              : await getCallLogPermission();
            setNativeCallLogPermission(permission);
            const snapshot =
              permission.status === "granted"
                ? await readNativeMissedCalls(
                    Date.now() - 30 * 24 * 60 * 60 * 1_000,
                    200,
                  )
                : { calls: [], platformVersion: permission.platformVersion };
            await withAuthenticatedSession((accessToken) =>
              synchronizeMissedCalls(apiBaseUrl, accessToken, {
                permission: permission.status,
                platformVersion: snapshot.platformVersion,
                calls: snapshot.calls.map((call) => ({
                  sourceId: call.sourceId,
                  occurredAt: new Date(
                    call.occurredAtEpochMillis,
                  ).toISOString(),
                  callerName: call.callerName,
                  phoneNumber: call.phoneNumber,
                })),
              }),
            );
          }
          const states = await withAuthenticatedSession((accessToken) =>
            fetchDeviceSignalStates(apiBaseUrl, accessToken),
          );
          setDeviceSignalStates(states);
          return true;
        } catch {
          setDeviceSignalsError(copy.settings.deviceSignalsLoadNotice);
          return false;
        } finally {
          setDeviceSignalsLoading(false);
        }
      })();
      deviceSignalSyncInFlightRef.current = operation;
      try {
        return await operation;
      } finally {
        if (deviceSignalSyncInFlightRef.current === operation) {
          deviceSignalSyncInFlightRef.current = undefined;
        }
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

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

  const loadGmailAccounts = useCallback(async (): Promise<
    GmailAccount[] | undefined
  > => {
    if (!tokens) return undefined;
    setGmailLoading(true);
    setGmailError(undefined);
    try {
      const response = await withAuthenticatedSession((accessToken) =>
        fetchGmailAccounts(apiBaseUrl, accessToken),
      );
      setGmailAccountsAvailable(response.available);
      setGmailAccounts(response.items);
      setGmailError(undefined);
      setGmailAuthorizationPending((pending) => {
        if (pending && gmailAuthorizationChanged(pending, response.items)) {
          return undefined;
        }
        return pending;
      });
      return response.items;
    } catch {
      setGmailError(copy.settings.gmailLoadRecovery);
      return undefined;
    } finally {
      setGmailLoading(false);
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const beginGmailConnection = useCallback(
    async (workspaceId: string, accountId?: string): Promise<void> => {
      if (
        !tokens ||
        gmailActions.some((action) => action.kind === "authorizing")
      ) {
        return;
      }
      const action: GmailAction = {
        kind: "authorizing",
        workspaceId,
        accountId,
      };
      setGmailActions((current) => [...current, action]);
      setGmailError(undefined);
      try {
        const authorization = await withAuthenticatedSession((accessToken) =>
          startGmailAuthorization(apiBaseUrl, accessToken, workspaceId, {
            accountId,
          }),
        );
        await openExternalUrl(authorization.authorizationUrl);
        setGmailAuthorizationPending({
          expiresAt: authorization.expiresAt,
          ...gmailAuthorizationBaseline(workspaceId, gmailAccounts, accountId),
        });
      } catch {
        setGmailError(
          gmailAccountsAvailable
            ? copy.settings.gmailConnectRecovery
            : copy.settings.gmailConfigurationMissing,
        );
      } finally {
        setGmailActions((current) =>
          current.filter((candidate) => candidate !== action),
        );
      }
    },
    [
      apiBaseUrl,
      gmailAccountsAvailable,
      gmailAccounts,
      gmailActions,
      tokens,
      withAuthenticatedSession,
    ],
  );

  const cancelGmailAuthorization = useCallback((): void => {
    setGmailAuthorizationPending(undefined);
    setGmailError(undefined);
  }, []);

  const syncGmailAccount = useCallback(
    async (accountId: string): Promise<void> => {
      if (
        !tokens ||
        gmailActions.some(
          (action) =>
            action.kind !== "authorizing" && action.accountId === accountId,
        )
      ) {
        return;
      }
      const action: GmailAction = { kind: "syncing", accountId };
      setGmailActions((current) => [...current, action]);
      setGmailError(undefined);
      try {
        await withAuthenticatedSession((accessToken) =>
          synchronizeGmailAccount(apiBaseUrl, accessToken, accountId),
        );
        await loadGmailAccounts();
      } catch {
        const refreshedAccounts = await loadGmailAccounts();
        if (refreshedAccounts) {
          setGmailError(copy.settings.gmailSyncRecovery);
        }
      } finally {
        setGmailActions((current) =>
          current.filter((candidate) => candidate !== action),
        );
      }
    },
    [
      apiBaseUrl,
      gmailActions,
      loadGmailAccounts,
      tokens,
      withAuthenticatedSession,
    ],
  );

  const disconnectGmailConnection = useCallback(
    async (accountId: string, expectedVersion: number): Promise<boolean> => {
      if (
        !tokens ||
        gmailActions.some(
          (action) =>
            action.kind !== "authorizing" && action.accountId === accountId,
        )
      ) {
        return false;
      }
      const action: GmailAction = { kind: "disconnecting", accountId };
      setGmailActions((current) => [...current, action]);
      setGmailError(undefined);
      try {
        await withAuthenticatedSession((accessToken) =>
          disconnectGmailAccount(
            apiBaseUrl,
            accessToken,
            accountId,
            expectedVersion,
          ),
        );
        await loadGmailAccounts();
        return true;
      } catch {
        setGmailError(copy.settings.gmailDisconnectRecovery);
        return false;
      } finally {
        setGmailActions((current) =>
          current.filter((candidate) => candidate !== action),
        );
      }
    },
    [
      apiBaseUrl,
      gmailActions,
      loadGmailAccounts,
      tokens,
      withAuthenticatedSession,
    ],
  );

  const loadGoogleChatAccounts = useCallback(async (): Promise<
    GoogleChatAccount[] | undefined
  > => {
    if (!tokens) return undefined;
    try {
      const connection = await withAuthenticatedSession((accessToken) =>
        fetchGoogleChatConnections(apiBaseUrl, accessToken),
      );
      setGoogleChatAccountsAvailable(connection.available);
      setGoogleChatAccounts(connection.items);
      if (connection.items.some((account) => account.status === "active")) {
        setGoogleChatAuthorizationExpiresAt(undefined);
      }
      return connection.items;
    } catch {
      setInflowError(copy.projects.inflowLoadProblem);
      return undefined;
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession]);

  const beginGoogleChatConnection = useCallback(async (): Promise<void> => {
    if (!tokens || inflowSaving) return;
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      const authorization = await withAuthenticatedSession((accessToken) =>
        startGoogleChatAuthorization(apiBaseUrl, accessToken),
      );
      await openExternalUrl(authorization.authorizationUrl);
      setGoogleChatAuthorizationExpiresAt(authorization.expiresAt);
    } catch {
      setInflowError(copy.projects.inflowSourceProblem);
    } finally {
      setInflowSaving(false);
    }
  }, [apiBaseUrl, inflowSaving, tokens, withAuthenticatedSession]);

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

  const loadGmailInflow = useCallback(async (): Promise<void> => {
    if (!tokens) return;
    if (workspaces.length === 0) {
      setGmailInflowItems([]);
      setGmailInflowProjects([]);
      setGmailInflowCursors({});
      setGmailInflowError(undefined);
      setGmailInflowLoadHealth(emptyGmailInflowLoadHealth);
      setGmailInflowLoading(false);
      setGmailInflowLoadingMore(false);
      return;
    }
    const requestGeneration = gmailInflowRequestGateRef.current.begin();
    setGmailInflowLoading(true);
    setGmailInflowLoadingMore(false);
    setGmailInflowCursors({});
    setGmailInflowError(undefined);
    try {
      const results = await Promise.all(
        workspaces.map(async (workspace) => {
          const [inflow, workspaceProjects] = await Promise.allSettled([
            withAuthenticatedSession((accessToken) =>
              fetchGmailInflow(apiBaseUrl, accessToken, workspace.id),
            ),
            withAuthenticatedSession((accessToken) =>
              fetchProjects(apiBaseUrl, accessToken, workspace.id),
            ),
          ]);
          return { workspace, inflow, workspaceProjects };
        }),
      );
      if (!gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        return;
      }
      const nextItems = results.flatMap(({ inflow }) =>
        inflow.status === "fulfilled" ? inflow.value.items : [],
      );
      const nextProjects = results.flatMap(({ workspaceProjects }) =>
        workspaceProjects.status === "fulfilled" ? workspaceProjects.value : [],
      );
      const initialFailedWorkspaces = results.flatMap(
        ({ workspace, inflow, workspaceProjects }) =>
          inflow.status === "rejected" ||
          workspaceProjects.status === "rejected" ||
          (inflow.status === "fulfilled" && inflow.value.partial)
            ? [workspace.name]
            : [],
      );
      setGmailInflowItems(
        [...nextItems].sort(
          (left, right) =>
            (Date.parse(right.receivedAt) || 0) -
            (Date.parse(left.receivedAt) || 0),
        ),
      );
      const nextCursors: Record<string, string | null> = {};
      for (const { workspace, inflow } of results) {
        if (inflow.status === "fulfilled") {
          nextCursors[workspace.id] = inflow.value.nextCursor;
        }
      }
      setGmailInflowCursors(nextCursors);
      setGmailInflowProjects(
        Array.from(
          new Map(
            nextProjects.map((project) => [project.id, project]),
          ).values(),
        ),
      );
      setGmailInflowLoadHealth(
        gmailInflowHealthAfterInitial(initialFailedWorkspaces),
      );
      setGmailInflowError(undefined);
    } catch {
      if (gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        setGmailInflowLoadHealth(
          gmailInflowHealthAfterInitial(
            workspaces.map((workspace) => workspace.name),
          ),
        );
        setGmailInflowError(copy.gmailInflow.loadProblem);
      }
    } finally {
      if (gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        setGmailInflowLoading(false);
      }
    }
  }, [apiBaseUrl, tokens, withAuthenticatedSession, workspaces]);

  const loadMoreGmailInflow = useCallback(async (): Promise<void> => {
    if (!tokens || gmailInflowLoadingMore) return;
    const pendingPages = workspaces.flatMap((workspace) => {
      const cursor = gmailInflowCursors[workspace.id];
      return cursor ? [{ workspace, cursor }] : [];
    });
    if (pendingPages.length === 0) return;
    const requestGeneration = gmailInflowRequestGateRef.current.begin();
    setGmailInflowLoadingMore(true);
    setGmailInflowLoadHealth((current) =>
      gmailInflowHealthAfterLoadMore(current, []),
    );
    try {
      const results = await Promise.all(
        pendingPages.map(async ({ workspace, cursor }) => ({
          workspace,
          cursor,
          page: await Promise.resolve(
            withAuthenticatedSession((accessToken) =>
              fetchGmailInflow(apiBaseUrl, accessToken, workspace.id, cursor),
            ),
          ).then(
            (value) => ({ status: "fulfilled" as const, value }),
            (reason: unknown) => ({ status: "rejected" as const, reason }),
          ),
        })),
      );
      if (!gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        return;
      }
      const loadedItems = results.flatMap(({ page }) =>
        page.status === "fulfilled" ? page.value.items : [],
      );
      setGmailInflowItems((current) =>
        Array.from(
          new Map(
            [...current, ...loadedItems].map((item) => [item.id, item]),
          ).values(),
        ).sort(
          (left, right) =>
            (Date.parse(right.receivedAt) || 0) -
            (Date.parse(left.receivedAt) || 0),
        ),
      );
      setGmailInflowCursors((current) => {
        const next = { ...current };
        for (const { workspace, cursor, page } of results) {
          if (page.status === "fulfilled") {
            next[workspace.id] =
              page.value.nextCursor === cursor ? cursor : page.value.nextCursor;
          }
        }
        return next;
      });
      const failedWorkspaces = results.flatMap(({ workspace, cursor, page }) =>
        page.status === "rejected" ||
        (page.status === "fulfilled" && page.value.nextCursor === cursor)
          ? [workspace.name]
          : [],
      );
      setGmailInflowLoadHealth((current) =>
        gmailInflowHealthAfterLoadMore(current, failedWorkspaces),
      );
    } catch {
      if (gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        setGmailInflowLoadHealth((current) =>
          gmailInflowHealthAfterLoadMore(
            current,
            pendingPages.map(({ workspace }) => workspace.name),
          ),
        );
      }
    } finally {
      if (gmailInflowRequestGateRef.current.isCurrent(requestGeneration)) {
        setGmailInflowLoadingMore(false);
      }
    }
  }, [
    apiBaseUrl,
    gmailInflowCursors,
    gmailInflowLoadingMore,
    tokens,
    withAuthenticatedSession,
    workspaces,
  ]);

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

  const loadWeeklyReport = useCallback(
    async (workspaceId: string) => {
      if (!tokens) return undefined;
      setWeeklyReportError(undefined);
      try {
        const { report, history } = await withAuthenticatedSession(
          async (accessToken) => {
            const [report, history] = await Promise.all([
              fetchWeeklyReport(apiBaseUrl, accessToken, workspaceId),
              fetchWeeklyReportHistory(
                apiBaseUrl,
                accessToken,
                workspaceId,
              ).catch(() => []),
            ]);
            return { report, history };
          },
        );
        setWeeklyReport(report);
        setWeeklyReportHistory(history);
        return report;
      } catch {
        setWeeklyReport(undefined);
        setWeeklyReportHistory([]);
        setWeeklyReportError(copy.projects.weeklyReportLoadProblem);
        return undefined;
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadProjectsForWorkspace = useCallback(
    async (workspaceId: string, preferredProjectId?: string) => {
      if (!tokens) return false;
      setProjectsLoading(true);
      setProjectsError(undefined);
      setWeeklyReportError(undefined);
      try {
        const result = await withAuthenticatedSession(async (accessToken) => {
          const [items, report, history] = await Promise.all([
            fetchProjects(apiBaseUrl, accessToken, workspaceId),
            fetchWeeklyReport(apiBaseUrl, accessToken, workspaceId).catch(
              () => undefined,
            ),
            fetchWeeklyReportHistory(
              apiBaseUrl,
              accessToken,
              workspaceId,
            ).catch(() => []),
          ]);
          return { items, report, history };
        });
        setProjects(result.items);
        setWeeklyReport(result.report);
        setWeeklyReportHistory(result.history);
        if (!result.report) {
          setWeeklyReportError(copy.projects.weeklyReportLoadProblem);
        }
        setSelectedProjectId((current) => {
          const next = preferredProjectId ?? current;
          return result.items.some((project) => project.id === next)
            ? next
            : undefined;
        });
        return true;
      } catch {
        setProjects([]);
        setWeeklyReport(undefined);
        setWeeklyReportHistory([]);
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

  const loadProjectReports = useCallback(
    async (workspaceId: string, projectId: string) => {
      if (!tokens) return undefined;
      setReportsLoading(true);
      setReportsError(undefined);
      try {
        const reports = await withAuthenticatedSession((accessToken) =>
          fetchProjectReports(apiBaseUrl, accessToken, workspaceId, projectId),
        );
        setProjectReports(reports);
        return reports;
      } catch {
        setProjectReports([]);
        setReportsError(copy.projects.reportLoadProblem);
        return undefined;
      } finally {
        setReportsLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const createProjectWeeklyReport = useCallback(
    async (workspaceId: string, projectId: string): Promise<void> => {
      if (!tokens) return;
      setReportsSaving(true);
      setReportsError(undefined);
      try {
        const report = await withAuthenticatedSession((accessToken) =>
          createProjectWeeklyReportRequest(
            apiBaseUrl,
            accessToken,
            workspaceId,
            projectId,
          ),
        );
        setProjectReports((current) => [
          report,
          ...current.filter((item) => item.id !== report.id),
        ]);
      } catch {
        setReportsError(copy.projects.reportGenerateProblem);
      } finally {
        setReportsSaving(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const updateProjectReport = useCallback(
    async (
      report: Report,
      content: ProjectWeeklyReportContent,
    ): Promise<void> => {
      if (!tokens) return;
      setReportsSaving(true);
      setReportsError(undefined);
      try {
        const updated = await withAuthenticatedSession((accessToken) =>
          updateReportRequest(apiBaseUrl, accessToken, report, content),
        );
        setProjectReports((current) =>
          current.map((item) => (item.id === updated.id ? updated : item)),
        );
      } catch {
        setReportsError(copy.projects.reportSaveProblem);
      } finally {
        setReportsSaving(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const finalizeProjectReport = useCallback(
    async (report: Report): Promise<void> => {
      if (!tokens) return;
      setReportsSaving(true);
      setReportsError(undefined);
      try {
        const finalized = await withAuthenticatedSession((accessToken) =>
          finalizeReportRequest(apiBaseUrl, accessToken, report),
        );
        setProjectReports((current) =>
          current.map((item) => (item.id === finalized.id ? finalized : item)),
        );
      } catch {
        setReportsError(copy.projects.reportFinalizeProblem);
      } finally {
        setReportsSaving(false);
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

  const loadProjectInflow = useCallback(
    async (projectId: string) => {
      if (!tokens) return undefined;
      setInflowLoading(true);
      setInflowError(undefined);
      try {
        const [sources, items] = await withAuthenticatedSession((accessToken) =>
          Promise.all([
            fetchProjectGoogleChatSources(apiBaseUrl, accessToken, projectId),
            fetchProjectInflow(apiBaseUrl, accessToken, projectId, "all"),
          ]),
        );
        setProjectGoogleChatSources(sources);
        setProjectInflowItems(items);
        return { sources, items };
      } catch {
        setProjectGoogleChatSources([]);
        setProjectInflowItems([]);
        setInflowError(copy.projects.inflowLoadProblem);
        return undefined;
      } finally {
        setInflowLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadGoogleChatSpaces = useCallback(
    async (accountId: string): Promise<void> => {
      if (!tokens || !accountId) return;
      setInflowLoading(true);
      setInflowError(undefined);
      try {
        setGoogleChatSpaces(
          await withAuthenticatedSession((accessToken) =>
            fetchGoogleChatSpaces(apiBaseUrl, accessToken, accountId),
          ),
        );
      } catch {
        setGoogleChatSpaces([]);
        setInflowError(copy.projects.inflowLoadProblem);
      } finally {
        setInflowLoading(false);
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const loadConversationMessages = useCallback(
    async (conversationId: string, background = false) => {
      if (!tokens || selectedConversationIdRef.current !== conversationId) {
        return;
      }
      const requestGeneration =
        conversationMessageRequestGateRef.current.begin();
      if (!background) {
        setConversationLoading(true);
        setConversationError(undefined);
      }
      try {
        const nextMessages = await withAuthenticatedSession((accessToken) =>
          fetchConversationMessages(apiBaseUrl, accessToken, conversationId),
        );
        if (
          conversationMessageRequestGateRef.current.isCurrent(
            requestGeneration,
          ) &&
          selectedConversationIdRef.current === conversationId
        ) {
          setConversationMessages(nextMessages);
        }
      } catch (error) {
        if (
          !background &&
          conversationMessageRequestGateRef.current.isCurrent(
            requestGeneration,
          ) &&
          selectedConversationIdRef.current === conversationId
        ) {
          setConversationMessages([]);
          setConversationError(
            error instanceof AgentRequestError && error.code === "notFound"
              ? copy.messages.conversationChanged
              : copy.messages.conversationLoadNotice,
          );
        }
      } finally {
        if (
          conversationMessageRequestGateRef.current.isCurrent(
            requestGeneration,
          ) &&
          selectedConversationIdRef.current === conversationId
        ) {
          setConversationLoading(false);
        }
      }
    },
    [apiBaseUrl, tokens, withAuthenticatedSession],
  );

  const refresh = useCallback(async () => {
    if (!sessionLoaded) return;
    if (!tokens) return;
    const conversationRequestGeneration =
      conversationListRequestGateRef.current.begin();
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
      ]);
      if (
        conversationListRequestGateRef.current.isCurrent(
          conversationRequestGeneration,
        )
      ) {
        setConversations(nextConversations);
      }
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
  }, [loadHomeSnapshot, sessionLoaded, tokens, withAuthenticatedSession]);

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
          "project_inflow_item",
          "project_inflow_analysis",
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
      const affectsGoogleChat =
        forceFull ||
        entityTypes.has("google_chat_account") ||
        entityTypes.has("project_google_chat_source") ||
        entityTypes.has("project_inflow_item") ||
        entityTypes.has("project_inflow_analysis");
      const affectsGmail =
        forceFull ||
        entityTypes.has("gmail_account") ||
        entityTypes.has("gmail_message") ||
        entityTypes.has("gmail_inflow_candidate");

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
        const conversationRequestGeneration =
          conversationListRequestGateRef.current.begin();
        const synchronizedConversations = await withAuthenticatedSession(
          (accessToken) => fetchConversations(apiBaseUrl, accessToken),
        );
        if (
          conversationListRequestGateRef.current.isCurrent(
            conversationRequestGeneration,
          )
        ) {
          setConversations(synchronizedConversations);
        }
        if (selectedConversationId) {
          await loadConversationMessages(selectedConversationId, true);
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
      if (affectsGoogleChat) {
        await loadGoogleChatAccounts();
        if (selectedProjectId) await loadProjectInflow(selectedProjectId);
      }
      if (affectsGmail) {
        await Promise.all([loadGmailAccounts(), loadGmailInflow()]);
      }
    },
    [
      apiBaseUrl,
      planningRange.from,
      planningRange.to,
      selectedConversationId,
      selectedProjectId,
      selectedWorkspaceId,
      loadGoogleChatAccounts,
      loadGmailAccounts,
      loadGmailInflow,
      loadProjectInflow,
      loadConversationMessages,
      withAuthenticatedSession,
    ],
  );

  const pullSyncChanges = useCallback(async (): Promise<void> => {
    if (!tokens) return;
    return syncPullCoordinatorRef.current.request(async () => {
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
    });
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
      setWeeklyReport(undefined);
      setWeeklyReportHistory([]);
      setGoals([]);
      setProjectTasks([]);
      setSelectedWorkspaceId(undefined);
      setSelectedProjectId(undefined);
      setHighlightedProjectTaskId(undefined);
      setHighlightedScheduleId(undefined);
      setHighlightedPlanningTaskId(undefined);
      setPlanningEditTarget(undefined);
      setProjectsError(undefined);
      setWeeklyReportError(undefined);
      setGoalsError(undefined);
      setConversationMessages([]);
      setCurrentConversationId(undefined);
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
      setGmailAccountsAvailable(true);
      setGmailAccounts([]);
      setGmailError(undefined);
      setGmailAuthorizationPending(undefined);
      setGmailActions([]);
      setGmailInflowItems([]);
      setGmailInflowProjects([]);
      setGmailInflowCursors({});
      setGmailInflowLoadingMore(false);
      setGmailInflowLoadHealth(emptyGmailInflowLoadHealth);
      setGmailInflowError(undefined);
      setGmailInflowSavingId(undefined);
      setReminderSyncStatus("idle");
      setReminderSyncError(undefined);
      setRemoteReminderStatus("idle");
      pendingConversationId.current = undefined;
      homeConversationDetachedRef.current = false;
      homeConversationStartingRef.current = false;
      conversationListRequestGateRef.current.invalidate();
      conversationMessageRequestGateRef.current.invalidate();
      gmailInflowRequestGateRef.current.invalidate();
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
    if (!tokens || homeConversationDetachedRef.current) return;
    const durableHome = conversations.find(
      (conversation) => conversation.surface === "home",
    );
    if (!durableHome) return;
    if (durableHome.id !== homeConversationId) {
      setHomeConversationId(durableHome.id);
    }
    if (destination !== "home") return;
    if (selectedConversationId === durableHome.id) return;
    setCurrentConversationId(durableHome.id);
    setConversationMessages([]);
    void Promise.all([
      loadConversationMessages(durableHome.id),
      restoreConversationJob(durableHome.id),
    ]);
  }, [
    conversations,
    destination,
    homeConversationId,
    loadConversationMessages,
    selectedConversationId,
    tokens,
  ]);

  useEffect(() => {
    if (!tokens || mode !== "ready") return;
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
  }, [apiBaseUrl, mode, pullSyncChanges, tokens, withAuthenticatedSession]);

  useLayoutEffect(() => {
    const frame = window.requestAnimationFrame(() => {
      window.scrollTo({
        top: 0,
        left: 0,
        behavior: "auto",
      });
    });
    return () => window.cancelAnimationFrame(frame);
  }, [destination]);

  useEffect(() => {
    if (!tokens || mode !== "ready") return;
    return runWhenBrowserIsIdle(() => {
      void synchronizePlanningReminders();
    });
  }, [mode, planningSnapshot, synchronizePlanningReminders, tokens]);

  useEffect(() => {
    if (!tokens || mode !== "ready") return;
    const refreshWhenVisible = () => {
      if (document.visibilityState === "visible") {
        void synchronizeDeviceSignals(false);
      }
    };
    const cancelInitialSync = runWhenBrowserIsIdle(() => {
      void synchronizeDeviceSignals(false);
    });
    window.addEventListener("focus", refreshWhenVisible);
    document.addEventListener("visibilitychange", refreshWhenVisible);
    return () => {
      cancelInitialSync();
      window.removeEventListener("focus", refreshWhenVisible);
      document.removeEventListener("visibilitychange", refreshWhenVisible);
    };
  }, [mode, synchronizeDeviceSignals, tokens]);

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
          const reminderDestination = reminderFallbackDestination(navigation);
          navigate(reminderDestination, {
            calendarDataReady: reminderDestination === "calendar",
          });
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
    if (destination !== "settings") return;
    void loadAgentModelSettings();
  }, [destination, loadAgentModelSettings]);

  useEffect(() => {
    const activation = calendarDestinationActivation(
      calendarDestinationActiveRef.current,
      destination === "calendar",
    );
    calendarDestinationActiveRef.current = activation.active;
    if (!activation.active) {
      calendarNavigationIntentRef.current = undefined;
    }
    if (activation.shouldLoad) {
      const intent = calendarNavigationIntentRef.current;
      calendarNavigationIntentRef.current = undefined;
      const planningLoad = calendarDestinationLoad(intent);
      void Promise.all([
        planningLoad.shouldLoadPlanning
          ? loadPlanningSnapshot(planningLoad.targetStartsAt)
          : Promise.resolve(undefined),
        loadGoogleCalendarConnection(),
      ]);
      return;
    }
    if (destination === "settings") {
      void loadGoogleCalendarConnection();
    }
  }, [destination, loadGoogleCalendarConnection, loadPlanningSnapshot]);

  useEffect(() => {
    if (destination !== "settings") return;
    void loadGmailAccounts();
  }, [destination, loadGmailAccounts]);

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
    if (!tokens || !gmailAuthorizationPending) return;
    let current = true;
    let pollInFlight = false;
    const expiresAt = new Date(gmailAuthorizationPending.expiresAt).getTime();
    const poll = async () => {
      if (pollInFlight) return;
      if (!Number.isFinite(expiresAt) || Date.now() >= expiresAt) {
        if (current) {
          setGmailAuthorizationPending(undefined);
          setGmailError(copy.settings.gmailAuthorizationExpired);
        }
        return;
      }
      pollInFlight = true;
      try {
        const response = await withAuthenticatedSession((accessToken) =>
          fetchGmailAccounts(apiBaseUrl, accessToken),
        );
        if (!current) return;
        setGmailAccountsAvailable(response.available);
        setGmailAccounts(response.items);
        setGmailError(undefined);
        const authorizationChanged = gmailAuthorizationChanged(
          gmailAuthorizationPending,
          response.items,
        );
        if (!response.available || authorizationChanged) {
          setGmailAuthorizationPending(undefined);
        }
      } catch {
        if (current) setGmailError(copy.settings.gmailLoadRecovery);
      } finally {
        pollInFlight = false;
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), 2_000);
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [apiBaseUrl, gmailAuthorizationPending, tokens, withAuthenticatedSession]);

  useEffect(() => {
    if (
      !tokens ||
      mode !== "ready" ||
      workspacesReady ||
      projectsLoading ||
      !["home", "projects", "meetings", "settings"].includes(destination)
    ) {
      return;
    }
    if (destination !== "home") {
      void loadWorkspaces();
      return;
    }
    return runWhenBrowserIsIdle(() => {
      void loadWorkspaces();
    });
  }, [
    destination,
    loadWorkspaces,
    mode,
    projectsLoading,
    tokens,
    workspacesReady,
  ]);

  useEffect(() => {
    if (!tokens || !workspacesReady || destination !== "home") return;
    void loadGmailInflow();
  }, [destination, loadGmailInflow, tokens, workspacesReady]);

  useEffect(() => {
    if (destination !== "projects") return;
    void loadGoals();
  }, [destination, loadGoals]);

  useEffect(() => {
    if (destination !== "projects") return;
    void loadGoogleChatAccounts();
  }, [destination, loadGoogleChatAccounts]);

  useEffect(() => {
    if (
      selectedWorkspaceId &&
      (destination === "projects" || destination === "meetings")
    ) {
      if (projectDataReadyOnNavigationRef.current) {
        projectDataReadyOnNavigationRef.current = false;
        return;
      }
      void loadProjectsForWorkspace(selectedWorkspaceId);
    }
  }, [destination, loadProjectsForWorkspace, selectedWorkspaceId]);

  useEffect(() => {
    if (selectedProjectId && destination === "projects") {
      void loadProjectTasks(selectedProjectId);
      if (selectedWorkspaceId) {
        void loadProjectReports(selectedWorkspaceId, selectedProjectId);
      }
      void loadProjectWebhooks(selectedProjectId);
      void loadProjectInflow(selectedProjectId);
    } else if (!selectedProjectId) {
      setProjectTasks([]);
      setProjectReports([]);
      setProjectWebhooks([]);
      setWebhookDeliveries([]);
      setProjectGoogleChatSources([]);
      setProjectInflowItems([]);
      setGoogleChatSpaces([]);
    }
  }, [
    loadProjectInflow,
    loadProjectReports,
    loadProjectTasks,
    loadProjectWebhooks,
    destination,
    selectedWorkspaceId,
    selectedProjectId,
  ]);

  useEffect(() => {
    if (!tokens || !googleChatAuthorizationExpiresAt) return;
    if (new Date(googleChatAuthorizationExpiresAt).getTime() <= Date.now()) {
      setGoogleChatAuthorizationExpiresAt(undefined);
      return;
    }
    let current = true;
    const poll = async () => {
      const accounts = await loadGoogleChatAccounts();
      if (current && accounts?.some((account) => account.status === "active")) {
        setGoogleChatAuthorizationExpiresAt(undefined);
      }
    };
    void poll();
    const interval = window.setInterval(() => void poll(), 2_000);
    return () => {
      current = false;
      window.clearInterval(interval);
    };
  }, [googleChatAuthorizationExpiresAt, loadGoogleChatAccounts, tokens]);

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
    navigate("chat");
    setAssistantDraft(undefined);
    setCurrentConversationId(conversationId);
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
    setCurrentConversationId(undefined);
    setConversationMessages([]);
    setConversationLoading(false);
    setConversationError(undefined);
    pendingConversationId.current = undefined;
  }

  async function startHomeConversation(): Promise<boolean> {
    if (!tokens || homeConversationStartingRef.current) return false;
    homeConversationStartingRef.current = true;
    homeConversationDetachedRef.current = true;
    conversationListRequestGateRef.current.invalidate();
    conversationMessageRequestGateRef.current.invalidate();
    setConversationLoading(true);
    setConversationError(undefined);
    const previousConversationId = homeConversationId;
    let previousConversationArchived = false;
    try {
      if (previousConversationId) {
        await withAuthenticatedSession((accessToken) =>
          archiveConversation(apiBaseUrl, accessToken, previousConversationId),
        );
        previousConversationArchived = true;
        conversationListRequestGateRef.current.invalidate();
        setConversations((current) =>
          current.filter(
            (conversation) => conversation.id !== previousConversationId,
          ),
        );
        setConversationJobs((current) => {
          const next = { ...current };
          delete next[previousConversationId];
          return next;
        });
        setHomeConversationId(undefined);
        startConversation();
      }

      const clientConversationId = createUuidV7();
      const conversation = await withAuthenticatedSession((accessToken) =>
        createConversation(
          apiBaseUrl,
          accessToken,
          clientConversationId,
          null,
          "home",
        ),
      );
      conversationListRequestGateRef.current.invalidate();
      conversationMessageRequestGateRef.current.invalidate();
      setConversations((current) => [
        conversation,
        ...current.filter(
          (known) => known.id !== conversation.id && known.surface !== "home",
        ),
      ]);
      setHomeConversationId(conversation.id);
      setCurrentConversationId(conversation.id);
      setConversationMessages([]);
      setAssistantDraft(undefined);
      pendingConversationId.current = undefined;
      homeConversationDetachedRef.current = false;
      return true;
    } catch {
      homeConversationDetachedRef.current = previousConversationArchived;
      setConversationError(copy.messages.conversationArchiveNotice);
      return false;
    } finally {
      setConversationLoading(false);
      homeConversationStartingRef.current = false;
    }
  }

  function openHomeAssistant() {
    if (!homeConversationId) {
      void openNewAssistantRequest();
      return;
    }
    setAssistantDraft(undefined);
    navigate("chat");
    if (selectedConversationId !== homeConversationId) {
      setCurrentConversationId(homeConversationId);
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
      applyCompletedTask(task, completed);
    } catch {
      setHomeError(copy.messages.taskCompletionNotice);
      setPlanningError(copy.messages.taskCompletionNotice);
      void loadHomeSnapshot();
      void loadPlanningSnapshot();
    }
  }

  function applyCompletedTask(task: Task, completed: Task) {
    void cancelLocalReminder("task", task.id).catch(() => false);
    setHomeSnapshot((current) =>
      current
        ? {
            ...current,
            tasks: current.tasks.filter((item) => item.id !== task.id),
            dueTasks: current.dueTasks.filter((item) => item.id !== task.id),
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
      if (selectedWorkspaceId) {
        void loadProjectsForWorkspace(selectedWorkspaceId, task.projectId);
      }
    }
    void loadGoals();
  }

  async function completeTaskFromAssistant(
    task: Pick<Task, "id" | "projectId">,
  ): Promise<Task> {
    setHomeError(undefined);
    try {
      const currentTask = await withAuthenticatedSession((accessToken) =>
        fetchTask(apiBaseUrl, accessToken, task.id),
      );
      if (currentTask.status !== "open") return currentTask;
      const completed = await withAuthenticatedSession((accessToken) =>
        completeTask(apiBaseUrl, accessToken, currentTask),
      );
      applyCompletedTask(currentTask, completed);
      return completed;
    } catch (error) {
      setHomeError(copy.messages.taskCompletionNotice);
      void loadHomeSnapshot();
      void loadPlanningSnapshot();
      throw error;
    }
  }

  async function loadTaskFromAssistant(
    task: Pick<Task, "id" | "projectId">,
  ): Promise<Task> {
    return withAuthenticatedSession((accessToken) =>
      fetchTask(apiBaseUrl, accessToken, task.id),
    );
  }

  async function editTaskFromAssistant(
    task: Pick<Task, "id" | "projectId">,
  ): Promise<void> {
    setHomeError(undefined);
    try {
      const currentTask = await withAuthenticatedSession((accessToken) =>
        fetchTask(apiBaseUrl, accessToken, task.id),
      );
      setPlanningEditTarget({ kind: "task", item: currentTask });
    } catch (error) {
      setHomeError(copy.messages.taskSaveNotice);
      throw error;
    }
  }

  async function editScheduleFromAssistant(
    entry: Pick<ScheduleEntry, "id" | "startsAt">,
  ): Promise<void> {
    setHomeError(undefined);
    const snapshot = await loadPlanningSnapshot(entry.startsAt);
    const currentEntry = snapshot?.schedule.find(
      (item) => item.id === entry.id,
    );
    if (!currentEntry) {
      setHomeError(copy.home.scheduleDestinationNotice);
      throw new Error("schedule unavailable");
    }
    setPlanningEditTarget({ kind: "schedule", item: currentEntry });
  }

  function applyRestoredTask(previous: Task, restored: Task) {
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
    setProjectTasks((current) => {
      const updated = current.map((item) =>
        item.id === restored.id ? restored : item,
      );
      return restored.projectId === selectedProjectId &&
        !updated.some((item) => item.id === restored.id)
        ? [restored, ...updated]
        : updated;
    });
    if (previous.projectId) {
      setProjects((current) =>
        current.map((project) =>
          project.id === previous.projectId
            ? { ...project, openTaskCount: project.openTaskCount + 1 }
            : project,
        ),
      );
      if (selectedWorkspaceId) {
        void loadProjectsForWorkspace(selectedWorkspaceId, previous.projectId);
      }
    }
    void loadHomeSnapshot();
    void loadGoals();
  }

  async function restoreTaskRecord(taskId: string): Promise<Task> {
    const currentTask = await withAuthenticatedSession((accessToken) =>
      fetchTask(apiBaseUrl, accessToken, taskId),
    );
    if (currentTask.status === "open") return currentTask;
    const restored = await withAuthenticatedSession((accessToken) =>
      updateTask(apiBaseUrl, accessToken, currentTask, {
        title: currentTask.title,
        notes: currentTask.notes ?? undefined,
        assigneeName: currentTask.assigneeName ?? undefined,
        status: "open",
        priority: currentTask.priority,
        dueAt: currentTask.dueAt ?? undefined,
        parentTaskId: currentTask.parentTaskId,
      }),
    );
    applyRestoredTask(currentTask, restored);
    return restored;
  }

  async function restoreTaskFromAssistant(
    task: Pick<Task, "id" | "projectId">,
  ): Promise<Task> {
    setHomeError(undefined);
    try {
      return await restoreTaskRecord(task.id);
    } catch (error) {
      setHomeError(copy.messages.taskRestoreNotice);
      void loadHomeSnapshot();
      void loadPlanningSnapshot();
      throw error;
    }
  }

  async function restorePlanningTask(task: Task): Promise<void> {
    if (!tokens) return;
    setPlanningError(undefined);
    try {
      await restoreTaskRecord(task.id);
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
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              tasks: [
                created,
                ...current.tasks.filter((item) => item.id !== created.id),
              ],
            }
          : current,
      );
      void Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
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
      setPlanningSnapshot((current) =>
        current
          ? {
              ...current,
              schedule: [
                ...current.schedule.filter((item) => item.id !== created.id),
                created,
              ].sort(
                (left, right) =>
                  new Date(left.startsAt).getTime() -
                  new Date(right.startsAt).getTime(),
              ),
            }
          : current,
      );
      void Promise.all([
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
      assigneeName?: string;
      status: Task["status"];
      priority: number;
      dueAt?: string;
    },
  ): Promise<void> {
    setPlanningError(undefined);
    const updated = await withAuthenticatedSession(async (accessToken) => {
      try {
        return await updateTask(apiBaseUrl, accessToken, task, input);
      } catch (error) {
        if (
          !(error instanceof PlanningRequestError) ||
          error.code !== "conflict"
        ) {
          throw error;
        }
        const latestTasks = task.projectId
          ? await fetchProjectTasks(apiBaseUrl, accessToken, task.projectId)
          : (
              await fetchPlanning(
                apiBaseUrl,
                accessToken,
                planningRange.from,
                planningRange.to,
              )
            ).tasks;
        const latest = latestTasks.find((item) => item.id === task.id);
        if (!latest) throw error;
        return updateTask(apiBaseUrl, accessToken, latest, input);
      }
    });
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
    void Promise.all([
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
      void Promise.all([
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
    void Promise.all([
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
    void Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
  }

  function selectWorkspace(workspaceId: string) {
    if (workspaceId === selectedWorkspaceId) return;
    setHighlightedProjectTaskId(undefined);
    setSelectedWorkspaceId(workspaceId);
    setSelectedProjectId(undefined);
    setProjectTasks([]);
    setProjectReports([]);
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
    navigate("projects", { projectDataReady: true });
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
      navigate("projects", { projectDataReady: true });
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
        navigate("projects", { projectDataReady: true });
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
    navigate("calendar", { calendarDataReady: true });
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
    navigate("calendar", { calendarDataReady: true });
  }

  async function createWorkspaceProject(input: {
    title: string;
    objective?: string;
    managementMode: Project["managementMode"];
    reportingEnabled: boolean;
    staleThresholdDays: number;
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
      managementMode: Project["managementMode"];
      reportingEnabled: boolean;
      staleThresholdDays: number;
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
      if (selectedWorkspaceId) void loadWeeklyReport(selectedWorkspaceId);
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

  async function createProjectTask(input: {
    title: string;
    parentTaskId?: string;
  }): Promise<void> {
    if (!selectedProjectId) throw new Error("project unavailable");
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const task = await withAuthenticatedSession((accessToken) =>
        createTask(apiBaseUrl, accessToken, {
          title: input.title,
          priority: 1,
          projectId: selectedProjectId,
          parentTaskId: input.parentTaskId,
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
      await Promise.all([
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId, selectedProjectId)
          : Promise.resolve(false),
        loadHomeSnapshot(),
        loadGoals(),
      ]);
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
      await Promise.all([
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId, selectedProjectId)
          : Promise.resolve(false),
        loadHomeSnapshot(),
        loadGoals(),
      ]);
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
      assigneeName?: string;
      status: Task["status"];
      priority: number;
      dueAt?: string;
      parentTaskId?: string | null;
    },
  ): Promise<void> {
    setProjectsSaving(true);
    setProjectsError(undefined);
    try {
      const updated = await withAuthenticatedSession(async (accessToken) => {
        try {
          return await updateTask(apiBaseUrl, accessToken, task, input);
        } catch (error) {
          if (
            !(error instanceof PlanningRequestError) ||
            error.code !== "conflict" ||
            !task.projectId
          ) {
            throw error;
          }
          const latest = (
            await fetchProjectTasks(apiBaseUrl, accessToken, task.projectId)
          ).find((item) => item.id === task.id);
          if (!latest) throw error;
          return updateTask(apiBaseUrl, accessToken, latest, input);
        }
      });
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
      await Promise.all([
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId, selectedProjectId)
          : Promise.resolve(false),
        loadHomeSnapshot(),
        loadPlanningSnapshot(),
        loadGoals(),
      ]);
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
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId, selectedProjectId)
          : Promise.resolve(false),
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
    mentionDirectory: WebhookMentionDirectory;
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
      mentionDirectory: WebhookMentionDirectory;
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

  async function createWorkspaceGoogleChatSource(input: {
    accountId: string;
    spaceName: string;
    displayName: string;
    acknowledgeWithReaction: boolean;
    importHistory: boolean;
  }): Promise<void> {
    if (!selectedProjectId) throw new Error("project unavailable");
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      const source = await withAuthenticatedSession((accessToken) =>
        createProjectGoogleChatSource(
          apiBaseUrl,
          accessToken,
          selectedProjectId,
          input,
        ),
      );
      setProjectGoogleChatSources((current) => [...current, source]);
      await syncWorkspaceGoogleChatSource(source);
    } catch (error) {
      setInflowError(copy.projects.inflowSourceProblem);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function deleteWorkspaceGoogleChatSource(
    source: ProjectGoogleChatSource,
  ): Promise<void> {
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        deleteProjectGoogleChatSource(apiBaseUrl, accessToken, source),
      );
      setProjectGoogleChatSources((current) =>
        current.filter((item) => item.id !== source.id),
      );
      setProjectInflowItems((current) =>
        current.filter((item) => item.sourceId !== source.id),
      );
    } catch (error) {
      setInflowError(copy.projects.inflowSourceProblem);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function syncWorkspaceGoogleChatSource(
    source: ProjectGoogleChatSource,
  ): Promise<void> {
    setInflowLoading(true);
    setInflowError(undefined);
    try {
      const sources = await withAuthenticatedSession((accessToken) =>
        syncProjectGoogleChatSource(apiBaseUrl, accessToken, source),
      );
      setProjectGoogleChatSources(sources);
      await loadProjectInflow(source.projectId);
    } catch (error) {
      setInflowError(googleChatSyncProblem(error));
      await loadGoogleChatAccounts();
      throw error;
    } finally {
      setInflowLoading(false);
    }
  }

  async function promoteWorkspaceInflow(
    item: ProjectInflowItem,
    input: PromoteInflowInput,
  ): Promise<void> {
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideProjectInflow(apiBaseUrl, accessToken, item, {
          decision: "promote",
          ...input,
        }),
      );
      setProjectInflowItems((current) =>
        current.filter(
          (currentItem) =>
            inflowConversationKey(currentItem) !== inflowConversationKey(item),
        ),
      );
      await loadHomeSnapshot();
      if (selectedProjectId === item.projectId) {
        await Promise.all([
          loadProjectTasks(item.projectId),
          loadProjectInflow(item.projectId),
          selectedWorkspaceId
            ? loadProjectsForWorkspace(selectedWorkspaceId, item.projectId)
            : Promise.resolve(false),
        ]);
      }
    } catch (error) {
      setInflowError(copy.projects.inflowDecisionProblem);
      await loadHomeSnapshot().catch(() => undefined);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function dismissWorkspaceInflow(
    item: ProjectInflowItem,
  ): Promise<void> {
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideProjectInflow(apiBaseUrl, accessToken, item, {
          decision: "dismiss",
        }),
      );
      setProjectInflowItems((current) =>
        current.filter(
          (currentItem) =>
            inflowConversationKey(currentItem) !== inflowConversationKey(item),
        ),
      );
      await loadHomeSnapshot();
      if (selectedProjectId === item.projectId) {
        await loadProjectInflow(item.projectId);
      }
    } catch (error) {
      setInflowError(copy.projects.inflowDecisionProblem);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function retryWorkspaceInflowCompletion(
    item: ProjectInflowItem,
  ): Promise<void> {
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideProjectInflow(apiBaseUrl, accessToken, item, {
          decision: "retry_completion",
        }),
      );
      await loadProjectInflow(item.projectId);
    } catch (error) {
      setInflowError(copy.projects.inflowDecisionProblem);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function retryWorkspaceInflowAnalysis(
    item: ProjectInflowItem,
  ): Promise<void> {
    setInflowSaving(true);
    setInflowError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideProjectInflow(apiBaseUrl, accessToken, item, {
          decision: "retry_analysis",
        }),
      );
      await Promise.all([
        loadHomeSnapshot(),
        selectedProjectId === item.projectId
          ? loadProjectInflow(item.projectId)
          : Promise.resolve(),
      ]);
    } catch (error) {
      setInflowError(copy.projects.inflowDecisionProblem);
      throw error;
    } finally {
      setInflowSaving(false);
    }
  }

  async function promoteGmailInflow(
    candidate: GmailInflowCandidate,
    input: PromoteGmailInflowInput,
  ): Promise<void> {
    if (gmailInflowSavingId) return;
    setGmailInflowSavingId(candidate.id);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideGmailInflow(apiBaseUrl, accessToken, candidate, {
          decision: "promote",
          ...input,
        }),
      );
      setGmailInflowItems((current) =>
        current.filter((item) => item.id !== candidate.id),
      );
      await Promise.all([loadHomeSnapshot(), loadPlanningSnapshot()]);
    } finally {
      setGmailInflowSavingId(undefined);
    }
  }

  async function dismissGmailInflow(
    candidate: GmailInflowCandidate,
  ): Promise<void> {
    if (gmailInflowSavingId) return;
    setGmailInflowSavingId(candidate.id);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideGmailInflow(apiBaseUrl, accessToken, candidate, {
          decision: "dismiss",
        }),
      );
      setGmailInflowItems((current) =>
        current.filter((item) => item.id !== candidate.id),
      );
    } finally {
      setGmailInflowSavingId(undefined);
    }
  }

  async function deferGmailInflow(
    candidate: GmailInflowCandidate,
    revisitAt: string,
  ): Promise<void> {
    if (gmailInflowSavingId) return;
    setGmailInflowSavingId(candidate.id);
    try {
      await withAuthenticatedSession((accessToken) =>
        decideGmailInflow(apiBaseUrl, accessToken, candidate, {
          decision: "defer",
          revisitAt,
        }),
      );
      setGmailInflowItems((current) =>
        current.filter((item) => item.id !== candidate.id),
      );
    } finally {
      setGmailInflowSavingId(undefined);
    }
  }

  async function retryGmailInflowAnalysis(
    candidate: GmailInflowCandidate,
  ): Promise<void> {
    if (gmailInflowSavingId) return;
    setGmailInflowSavingId(candidate.id);
    try {
      const updated = await withAuthenticatedSession((accessToken) =>
        decideGmailInflow(apiBaseUrl, accessToken, candidate, {
          decision: "retry_analysis",
        }),
      );
      setGmailInflowItems((current) =>
        updated.analysisStatus === "ready" ||
        updated.analysisStatus === "failed"
          ? current.map((item) => (item.id === updated.id ? updated : item))
          : current.filter((item) => item.id !== updated.id),
      );
    } finally {
      setGmailInflowSavingId(undefined);
    }
  }

  async function openNewAssistantRequest(): Promise<void> {
    setAssistantDraft(undefined);
    navigate("chat");
    await startHomeConversation();
  }

  async function handleVoiceTranscript(value: string): Promise<void> {
    const started = await startHomeConversation();
    if (!started) return;
    setAssistantDraft({ id: createUuidV7(), text: value, autoSend: true });
    navigate("chat");
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
            options.rememberForHome ? "home" : "chat",
          ),
        );
        pendingConversationId.current = undefined;
        conversationId = conversation.id;
        conversationListRequestGateRef.current.invalidate();
        setConversations((current) => [
          conversation,
          ...current.filter(
            (known) =>
              known.id !== conversation.id &&
              !(conversation.surface === "home" && known.surface === "home"),
          ),
        ]);
        setCurrentConversationId(conversation.id);
      }
      if (!conversationId) {
        setConversationError(copy.messages.conversationSendNotice);
        return false;
      }
      const targetConversationId = conversationId;
      if (selectedConversationId !== targetConversationId) {
        setCurrentConversationId(targetConversationId);
        setConversationMessages([]);
      }
      if (options.rememberForHome) {
        homeConversationDetachedRef.current = false;
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

  async function refreshCurrentDestination(): Promise<void> {
    if (destination === "decisions") {
      await loadDecisionInbox();
      return;
    }
    if (destination === "calendar") {
      await Promise.all([
        loadPlanningSnapshot(),
        loadGoogleCalendarConnection(),
      ]);
      return;
    }
    if (destination === "projects") {
      await Promise.all([
        loadGoals(),
        loadGoogleChatAccounts(),
        selectedWorkspaceId
          ? loadProjectsForWorkspace(selectedWorkspaceId, selectedProjectId)
          : loadWorkspaces(),
        selectedProjectId
          ? Promise.all([
              loadProjectTasks(selectedProjectId),
              loadProjectWebhooks(selectedProjectId),
              loadProjectInflow(selectedProjectId),
            ])
          : Promise.resolve(),
      ]);
      return;
    }
    if (destination === "settings") {
      await Promise.all([
        loadAgentModelSettings(),
        loadGoogleCalendarConnection(),
        loadGmailAccounts(),
        synchronizeDeviceSignals(false),
      ]);
      return;
    }
    await Promise.all([
      refresh(),
      destination === "home" && workspacesReady
        ? loadGmailInflow()
        : Promise.resolve(),
    ]);
  }

  function navigate(
    nextDestination: OsDestination,
    options: {
      projectDataReady?: boolean;
      calendarDataReady?: boolean;
    } = {},
  ): void {
    if (
      nextDestination !== destination &&
      document.querySelector('.meeting-transcript-editor[data-dirty="true"]') &&
      !window.confirm(copy.meetings.transcriptDiscardConfirm)
    ) {
      return;
    }
    if (nextDestination !== destination) {
      navigationHistoryRef.current = [
        ...navigationHistoryRef.current.slice(-31),
        destination,
      ];
    }
    setDestination(nextDestination);
    if (
      nextDestination === "home" &&
      homeConversationId &&
      selectedConversationId !== homeConversationId
    ) {
      setCurrentConversationId(homeConversationId);
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
      const intent: CalendarNavigationIntent = {
        planningReady: options.calendarDataReady === true,
        targetStartsAt: latestSchedule?.startsAt,
      };
      calendarNavigationIntentRef.current = intent;
      if (nextDestination === destination) {
        const planningLoad = calendarDestinationLoad(intent);
        calendarNavigationIntentRef.current = undefined;
        if (planningLoad.shouldLoadPlanning) {
          void loadPlanningSnapshot(planningLoad.targetStartsAt);
        }
      }
      return;
    }
    if (nextDestination === "projects") {
      if (options.projectDataReady) {
        projectDataReadyOnNavigationRef.current =
          nextDestination !== destination;
        return;
      }
      const latestProject = [
        ...(latestAssistantMessage?.presentation?.items ?? []),
      ]
        .reverse()
        .find((item) => item.type === "project");
      if (latestProject) {
        setSelectedWorkspaceId(latestProject.workspaceId);
        setSelectedProjectId(latestProject.id);
      }
    }
    if (nextDestination === "decisions") {
      void loadDecisionInbox();
    }
  }

  useEffect(() => installAndroidBackBridge(), []);

  useEffect(
    () =>
      registerMobileBackHandler(() => {
        if (planningEditTarget) {
          setPlanningEditTarget(undefined);
          return true;
        }
        if (destination === "projects" && selectedProjectId) {
          setHighlightedProjectTaskId(undefined);
          setSelectedProjectId(undefined);
          setProjectTasks([]);
          setProjectWebhooks([]);
          setWebhookDeliveries([]);
          setProjectGoogleChatSources([]);
          setProjectInflowItems([]);
          setGoogleChatSpaces([]);
          return true;
        }
        const previousDestination = navigationHistoryRef.current.pop();
        if (previousDestination) {
          setDestination(previousDestination);
          return true;
        }
        if (destination !== "home") {
          setDestination("home");
          return true;
        }
        return false;
      }, 10),
    [destination, planningEditTarget, selectedProjectId],
  );

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
          onRefresh={() => void refreshCurrentDestination()}
          refreshing={
            mode === "loading" ||
            (destination === "home" && homeLoading) ||
            (destination === "calendar" && planningLoading) ||
            (destination === "projects" &&
              (projectsLoading || goalsLoading || inflowLoading)) ||
            (destination === "decisions" && decisionsLoading)
          }
        >
          <WorkspaceRouteBoundary
            key={destination}
            loadingFallback={<WorkspaceRouteFallback />}
            onRetry={() => window.location.reload()}
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
                onOpenPlanning={() => navigate("calendar")}
                onStartNewAssistant={startHomeConversation}
                onSendAssistant={(text, clientMessageId) =>
                  sendConversationRequest(text, clientMessageId, {
                    startFresh: !homeConversationId,
                    targetConversationId: homeConversationId,
                    rememberForHome: true,
                  })
                }
                onCompleteTask={completeHomeTask}
                onLoadAssistantTask={loadTaskFromAssistant}
                onCompleteAssistantTask={completeTaskFromAssistant}
                onRestoreAssistantTask={restoreTaskFromAssistant}
                onEditAssistantTask={editTaskFromAssistant}
                onEditAssistantSchedule={editScheduleFromAssistant}
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
                onOpenMeetings={() => navigate("meetings")}
                onOpenSettings={() => navigate("settings")}
                onDecideRecommendation={decideHomeRecommendation}
                inflowSaving={inflowSaving}
                onPromoteInflow={promoteWorkspaceInflow}
                onDismissInflow={dismissWorkspaceInflow}
                onRetryInflowAnalysis={retryWorkspaceInflowAnalysis}
                onRetryInflowCompletion={retryWorkspaceInflowCompletion}
                gmailInflowItems={gmailInflowItems}
                gmailInflowProjects={gmailInflowProjects}
                gmailInflowLoading={gmailInflowLoading}
                gmailInflowLoadingMore={gmailInflowLoadingMore}
                gmailInflowLoadMoreError={
                  gmailInflowLoadHealth.initialFailedWorkspaces.length === 0 &&
                  gmailInflowLoadHealth.loadMoreFailedWorkspaces.length > 0
                }
                gmailInflowHasMore={Object.values(gmailInflowCursors).some(
                  Boolean,
                )}
                gmailInflowError={
                  gmailInflowError ??
                  (gmailInflowLoadHealth.initialFailedWorkspaces.length > 0
                    ? copy.gmailInflow.initialPartialProblem(
                        gmailInflowLoadHealth.initialFailedWorkspaces,
                      )
                    : gmailInflowLoadHealth.loadMoreFailedWorkspaces.length > 0
                      ? copy.gmailInflow.moreLoadProblem
                      : undefined)
                }
                gmailInflowSavingId={gmailInflowSavingId}
                onReloadGmailInflow={loadGmailInflow}
                onLoadMoreGmailInflow={loadMoreGmailInflow}
                onPromoteGmailInflow={promoteGmailInflow}
                onDismissGmailInflow={dismissGmailInflow}
                onDeferGmailInflow={deferGmailInflow}
                onRetryGmailInflowAnalysis={retryGmailInflowAnalysis}
              />
            )}
            {destination === "calendar" && (
              <PlanningWorkspace
                snapshot={planningSnapshot}
                range={planningRange}
                calendarConnection={calendarConnection}
                loading={planningLoading || mode === "loading"}
                error={
                  planningError ?? (mode === "error" ? message : undefined)
                }
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
                weeklyReport={weeklyReport}
                weeklyReportHistory={weeklyReportHistory}
                tasks={projectTasks}
                reports={projectReports}
                reportsLoading={reportsLoading}
                reportsSaving={reportsSaving}
                reportsError={reportsError}
                webhooks={projectWebhooks}
                webhookDeliveries={webhookDeliveries}
                googleChatAccountsAvailable={googleChatAccountsAvailable}
                googleChatAccounts={googleChatAccounts}
                googleChatSpaces={googleChatSpaces}
                googleChatSources={projectGoogleChatSources}
                projectInflowItems={projectInflowItems}
                selectedWorkspaceId={selectedWorkspaceId}
                selectedProjectId={selectedProjectId}
                highlightedTaskId={highlightedProjectTaskId}
                loaded={workspacesReady}
                loading={projectsLoading || goalsLoading || mode === "loading"}
                webhookLoading={webhooksLoading}
                inflowLoading={inflowLoading}
                saving={projectsSaving || goalsSaving || inflowSaving}
                error={goalsError ?? projectsError}
                weeklyReportError={weeklyReportError}
                inflowError={inflowError}
                onSelectWorkspace={selectWorkspace}
                onSelectProject={selectProject}
                onOpenGoalTask={(taskId, projectId) =>
                  void openTaskFromAssistant({ id: taskId, projectId })
                }
                onClearProject={() => {
                  setHighlightedProjectTaskId(undefined);
                  setSelectedProjectId(undefined);
                  setProjectTasks([]);
                  setProjectReports([]);
                  setProjectWebhooks([]);
                  setWebhookDeliveries([]);
                  setProjectGoogleChatSources([]);
                  setProjectInflowItems([]);
                  setGoogleChatSpaces([]);
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
                onCreateWeeklyReport={createProjectWeeklyReport}
                onUpdateReport={updateProjectReport}
                onFinalizeReport={finalizeProjectReport}
                onCreateWebhook={createWorkspaceWebhook}
                onUpdateWebhook={updateWorkspaceWebhook}
                onTestWebhook={testWorkspaceWebhook}
                onDeleteWebhook={deleteWorkspaceWebhook}
                onRetryWebhookDelivery={retryWorkspaceWebhookDelivery}
                onConnectGoogleChatAccount={beginGoogleChatConnection}
                onLoadGoogleChatSpaces={loadGoogleChatSpaces}
                onCreateGoogleChatSource={createWorkspaceGoogleChatSource}
                onDeleteGoogleChatSource={deleteWorkspaceGoogleChatSource}
                onSyncGoogleChatSource={syncWorkspaceGoogleChatSource}
                onPromoteInflow={promoteWorkspaceInflow}
                onDismissInflow={dismissWorkspaceInflow}
                onRetryInflowAnalysis={retryWorkspaceInflowAnalysis}
                onRetryInflowCompletion={retryWorkspaceInflowCompletion}
              />
            )}
            {destination === "decisions" && (
              <DecisionInboxWorkspace
                recommendations={decisionRecommendations}
                loading={decisionsLoading || mode === "loading"}
                error={decisionsError}
                onOpenConversation={selectConversation}
                onDecide={decideHomeRecommendation}
              />
            )}
            {destination === "meetings" && tokens && (
              <MeetingsWorkspace
                apiBaseUrl={apiBaseUrl}
                accessToken={tokens.accessToken}
                workspaces={workspaces}
                projects={projects}
                selectedWorkspaceId={selectedWorkspaceId}
                onSelectWorkspace={selectWorkspace}
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
                workspaces={workspaces}
                gmailAvailable={gmailAccountsAvailable}
                gmailAccounts={gmailAccounts}
                gmailLoading={gmailLoading}
                gmailActions={gmailActions}
                gmailAuthorizationPendingWorkspaceId={
                  gmailAuthorizationPending?.workspaceId
                }
                gmailError={gmailError}
                reminderSyncStatus={reminderSyncStatus}
                reminderSyncError={reminderSyncError}
                remoteReminderStatus={remoteReminderStatus}
                deviceSignalStates={deviceSignalStates}
                nativeCallLogPermission={nativeCallLogPermission}
                deviceSignalsLoading={deviceSignalsLoading}
                deviceSignalsError={deviceSignalsError}
                onStartAuthentication={beginAgentAuthentication}
                onReloadModels={loadAgentModelSettings}
                onSaveModel={saveAgentModelSettings}
                onStartCalendarConnection={beginGoogleCalendarConnection}
                onReloadCalendarConnection={loadGoogleCalendarConnection}
                onSyncCalendar={syncGoogleCalendar}
                onDisconnectCalendar={disconnectGoogleCalendarConnection}
                onReloadGmailAccounts={loadGmailAccounts}
                onStartGmailConnection={beginGmailConnection}
                onCancelGmailAuthorization={cancelGmailAuthorization}
                onSyncGmailAccount={syncGmailAccount}
                onDisconnectGmailAccount={disconnectGmailConnection}
                onRetryReminderSync={synchronizePlanningReminders}
                onEnableDeviceSignals={() => synchronizeDeviceSignals(true)}
                onRefreshDeviceSignals={() => synchronizeDeviceSignals(false)}
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
                onStartConversation={startHomeConversation}
                onStartAuthentication={beginAgentAuthentication}
                onSend={sendConversationRequest}
                onResolveAction={resolveConversationAction}
              />
            )}
          </WorkspaceRouteBoundary>
          {planningEditTarget && (
            <Suspense fallback={null}>
              <PlanningItemEditor
                target={planningEditTarget}
                onClose={() => setPlanningEditTarget(undefined)}
                onSaveTask={savePlanningTask}
                onSaveSchedule={savePlanningSchedule}
                onDeleteTask={deletePlanningTask}
                onDeleteSchedule={deletePlanningSchedule}
              />
            </Suspense>
          )}
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

function WorkspaceRouteFallback() {
  return (
    <section className="workspace-route-fallback" aria-busy="true">
      <span className="workspace-route-fallback__heading" aria-hidden="true" />
      <span className="workspace-route-fallback__summary" aria-hidden="true" />
      <div className="workspace-route-fallback__surface" aria-hidden="true">
        <span />
        <span />
        <span />
      </div>
      <span className="sr-only" role="status" aria-live="polite">
        {copy.launch.loading}
      </span>
    </section>
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

function googleChatSyncProblem(error: unknown): string {
  if (!(error instanceof GoogleChatRequestError)) {
    return copy.projects.inflowSyncProblem;
  }
  if (
    error.serverCode === "google_chat.authorization_rejected" ||
    error.serverCode === "google_chat.required_scope_missing"
  ) {
    return copy.projects.inflowReconnectProblem;
  }
  return error.retryable
    ? copy.projects.inflowSyncProblem
    : copy.projects.inflowLoadProblem;
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

function runWhenBrowserIsIdle(work: () => void, timeout = 1_000): () => void {
  if (typeof window.requestIdleCallback === "function") {
    const idleCallback = window.requestIdleCallback(work, { timeout });
    return () => window.cancelIdleCallback(idleCallback);
  }
  const timer = globalThis.setTimeout(work, 160);
  return () => globalThis.clearTimeout(timer);
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
