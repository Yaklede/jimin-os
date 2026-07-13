import { Server, Sparkles } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  bootstrapTrustedNetworkSession,
  completeTask,
  createTask,
  refreshDeviceSession,
  updateTask,
  type ScheduleEntry,
  type SessionTokens,
  type Task,
} from "./api/planning";
import {
  createProject,
  fetchProjects,
  fetchProjectTasks,
  fetchWorkspaces,
  updateProject,
  type Project,
  type Workspace,
} from "./api/projects";
import { type HomeSnapshot, fetchHomeSnapshot } from "./api/home";
import { processVoiceCommand } from "./api/voice";
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
import { ConversationWorkspace } from "./components/ConversationWorkspace";
import { AssistantRail, HomeWorkspace } from "./components/HomeWorkspace";
import { MemoryWorkspace } from "./components/MemoryWorkspace";
import { OsShell, type OsDestination } from "./components/OsShell";
import { PlanningWorkspace } from "./components/PlanningWorkspace";
import { ProjectsWorkspace } from "./components/ProjectsWorkspace";
import { SettingsWorkspace } from "./components/SettingsWorkspace";
import { type VoiceCommandOutcome } from "./components/VoiceCommandSheet";
import { copy } from "./copy";
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
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectTasks, setProjectTasks] = useState<Task[]>([]);
  const [selectedWorkspaceId, setSelectedWorkspaceId] = useState<string>();
  const [selectedProjectId, setSelectedProjectId] = useState<string>();
  const [highlightedProjectTaskId, setHighlightedProjectTaskId] =
    useState<string>();
  const [highlightedScheduleId, setHighlightedScheduleId] = useState<string>();
  const [projectsLoading, setProjectsLoading] = useState(false);
  const [projectsSaving, setProjectsSaving] = useState(false);
  const [projectsError, setProjectsError] = useState<string>();
  const [selectedConversationId, setSelectedConversationId] = useState<
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
  const pendingConversationId = useRef<string | undefined>(undefined);
  const activeSessionRef = useRef<SessionTokens | undefined>(undefined);
  const refreshInFlightRef = useRef<Promise<SessionTokens> | undefined>(
    undefined,
  );
  const [message, setMessage] = useState<string | undefined>(undefined);

  const applyActiveSession = useCallback((session: SessionTokens) => {
    activeSessionRef.current = session;
    setTokens(session);
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

      const refresh = refreshDeviceSession(apiBaseUrl, staleRefreshToken);
      refreshInFlightRef.current = refresh;
      try {
        const refreshed = await refresh;
        await persistActiveSession(refreshed);
        return refreshed;
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
      const session = await bootstrapTrustedNetworkSession(
        apiBaseUrl,
        copy.personalServer.deviceName,
        installationId,
      );
      await persistActiveSession(session);
    } catch {
      setMode("server-unreachable");
      setMessage(copy.messages.serverOffline);
    }
  }, [apiBaseUrl, persistActiveSession]);

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

  const loadWorkspaces = useCallback(async () => {
    if (!tokens) return;
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
    } catch {
      setProjectsError(copy.messages.projectsLoadNotice);
    } finally {
      setProjectsLoading(false);
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
            : items[0]?.id;
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
  }, [loadHomeSnapshot, sessionLoaded, tokens, withAuthenticatedSession]);

  async function discardSession() {
    try {
      await clearDeviceSession();
    } finally {
      activeSessionRef.current = undefined;
      setTokens(undefined);
      setConversations([]);
      setHomeSnapshot(undefined);
      setHomeError(undefined);
      setWorkspaces([]);
      setProjects([]);
      setProjectTasks([]);
      setSelectedWorkspaceId(undefined);
      setSelectedProjectId(undefined);
      setHighlightedProjectTaskId(undefined);
      setHighlightedScheduleId(undefined);
      setProjectsError(undefined);
      setConversationMessages([]);
      setSelectedConversationId(undefined);
      setAssistantDraft(undefined);
      setConversationJobs({});
      setAgentAuthentication(undefined);
      setAgentModelSettings(undefined);
      setAgentModelsError(undefined);
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
  }, [apiBaseUrl, applyActiveSession, bootstrapTrustedNetworkDevice]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    void loadAgentModelSettings();
  }, [loadAgentModelSettings]);

  useEffect(() => {
    void loadWorkspaces();
  }, [loadWorkspaces]);

  useEffect(() => {
    if (selectedWorkspaceId) {
      void loadProjectsForWorkspace(selectedWorkspaceId);
    }
  }, [loadProjectsForWorkspace, selectedWorkspaceId]);

  useEffect(() => {
    if (selectedProjectId) {
      void loadProjectTasks(selectedProjectId);
    } else {
      setProjectTasks([]);
    }
  }, [loadProjectTasks, selectedProjectId]);

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
    refreshConversations,
    selectedConversationId,
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

  async function completeHomeTask(task: Task): Promise<void> {
    if (!tokens) return;
    setHomeError(undefined);
    try {
      await withAuthenticatedSession((accessToken) =>
        completeTask(apiBaseUrl, accessToken, task),
      );
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
    entry: Pick<ScheduleEntry, "id">,
  ): Promise<void> {
    if (!(await loadHomeSnapshot())) {
      throw new Error("schedule destination unavailable");
    }
    setHighlightedScheduleId(entry.id);
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
      void loadHomeSnapshot();
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
    } catch {
      setProjectsError(copy.messages.projectTaskSaveNotice);
      if (selectedProjectId) void loadProjectTasks(selectedProjectId);
      throw new Error("task update failed");
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
    try {
      const result = await withAuthenticatedSession((accessToken) =>
        processVoiceCommand(apiBaseUrl, accessToken, value),
      );
      if (
        result.kind === "schedule_listed" ||
        result.kind === "schedule_created" ||
        result.kind === "tasks_listed" ||
        result.kind === "task_created"
      ) {
        void loadHomeSnapshot();
        return {
          kind: "handled",
          message: result.message,
          destination: result.destination === "calendar" ? "calendar" : "home",
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
    startFresh = false,
  ): Promise<boolean> {
    if (!tokens || agentAuthentication?.state !== "ready") {
      setConversationError(copy.messages.authenticationRequired);
      return false;
    }
    let conversationId = startFresh ? undefined : selectedConversationId;
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
  const latestAssistantMessage = [...conversationMessages]
    .reverse()
    .find((message) => message.role === "assistant");

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
          onNavigate={setDestination}
          onVoiceTranscript={handleVoiceTranscript}
          onVoiceCommand={handleVoiceCommand}
          onRefresh={() => void refresh()}
          refreshing={mode === "loading"}
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
                selectedConversationId
                  ? conversationJobs[selectedConversationId]
                  : undefined
              }
              assistantMessage={latestAssistantMessage}
              onOpenAssistant={openNewAssistantRequest}
              onSendAssistant={(text, clientMessageId) =>
                sendConversationRequest(text, clientMessageId, true)
              }
              onCompleteTask={completeHomeTask}
              onOpenTask={openTaskFromAssistant}
              onOpenProject={openProjectFromAssistant}
              onOpenSchedule={openScheduleFromAssistant}
            />
          )}
          {destination === "calendar" && (
            <PlanningWorkspace
              snapshot={homeSnapshot}
              loading={homeLoading || mode === "loading"}
              error={homeError ?? (mode === "error" ? message : undefined)}
              highlightedScheduleId={highlightedScheduleId}
              onCompleteTask={completeHomeTask}
            />
          )}
          {destination === "projects" && (
            <ProjectsWorkspace
              workspaces={workspaces}
              projects={projects}
              tasks={projectTasks}
              selectedWorkspaceId={selectedWorkspaceId}
              selectedProjectId={selectedProjectId}
              highlightedTaskId={highlightedProjectTaskId}
              loading={projectsLoading || mode === "loading"}
              saving={projectsSaving}
              error={projectsError}
              onSelectWorkspace={selectWorkspace}
              onSelectProject={selectProject}
              onCreateProject={createWorkspaceProject}
              onUpdateProject={updateWorkspaceProject}
              onCreateTask={createProjectTask}
              onCompleteTask={completeProjectTask}
              onUpdateTask={updateProjectTask}
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
              onStartAuthentication={beginAgentAuthentication}
              onReloadModels={loadAgentModelSettings}
              onSaveModel={saveAgentModelSettings}
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

function isTerminalAgentJob(state: AgentJob["state"]) {
  return ["completed", "failed", "cancelled", "declined"].includes(state);
}
