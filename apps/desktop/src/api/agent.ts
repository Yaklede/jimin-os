export interface Conversation {
  id: string;
  title: string | null;
  status: "active" | "archived";
  lastMessageAt: string | null;
  version: number;
}

export interface ConversationMessage {
  id: string;
  role: "user" | "assistant" | "system_event";
  content: string;
  presentation: AssistantPresentation | null;
  status: "pending" | "streaming" | "completed" | "failed" | "cancelled";
  createdAt: string;
  completedAt: string | null;
  version: number;
}

export interface AssistantPresentation {
  kind: "summary" | "tasks" | "schedule" | "projects" | "composite";
  title: string;
  items: AssistantPresentationItem[];
  layout: "stack" | "split" | "focus";
  sections: AssistantPresentationSection[];
  focusItemId: string | null;
}

export interface AssistantPresentationSection {
  kind: "tasks" | "schedule" | "projects";
  title: string;
  view: "list" | "checklist" | "timeline" | "cards";
  itemIds: string[];
}

export type AssistantPresentationItem =
  | {
      type: "task";
      id: string;
      projectId: string | null;
      projectTitle: string | null;
      title: string;
      priority: number;
      dueAt: string | null;
    }
  | {
      type: "schedule";
      id: string;
      title: string;
      startsAt: string;
      endsAt: string;
      timeZone: string;
    }
  | {
      type: "project";
      id: string;
      workspaceId: string;
      title: string;
      objective: string | null;
      nextAction: string | null;
      riskLevel: number;
      openTaskCount: number;
    };

export interface AgentJob {
  id: string;
  conversationId: string;
  state:
    | "queued"
    | "claimed"
    | "running"
    | "waiting_approval"
    | "retry_wait"
    | "completed"
    | "failed"
    | "cancelled"
    | "declined";
  createdAt: string;
  finishedAt: string | null;
  version: number;
  pendingAction: PendingAgentAction | null;
}

export interface PendingAgentAction {
  kind: "create_task" | "create_schedule";
  title: string;
  startsAt: string | null;
  endsAt: string | null;
}

export interface ConversationStreamSnapshot {
  messages: ConversationMessage[];
  job: AgentJob | null;
}

export interface QueuedAgentTurn {
  jobId: string;
  messageId: string;
  conversationId: string;
  state: AgentJob["state"];
}

export interface AgentAuthentication {
  state:
    "needs_login" | "requested" | "awaiting_authorization" | "ready" | "failed";
  verificationUrl: string | null;
  userCode: string | null;
}

export interface AgentModel {
  id: string;
  displayName: string;
  description: string;
  isDefault: boolean;
  defaultReasoningEffort: string;
  supportedReasoningEfforts: AgentReasoningEffort[];
}

export interface AgentReasoningEffort {
  id: string;
  description: string;
}

export interface AgentModelSettings {
  items: AgentModel[];
  selectedModelId: string | null;
  selectedReasoningEffort: string | null;
}

interface ListResponse<T> {
  items: T[];
  nextCursor: string | null;
}

export class AgentRequestError extends Error {
  readonly code:
    "unauthorized" | "invalid" | "conflict" | "notFound" | "unavailable";

  constructor(code: AgentRequestError["code"]) {
    super(code);
    this.name = "AgentRequestError";
    this.code = code;
  }
}

export async function fetchConversations(
  baseUrl: string,
  access: string,
): Promise<Conversation[]> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/conversations`,
    {
      headers: authHeaders(access),
    },
  );
  const body = await readJson(response);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isListResponse<Conversation>(body)) {
    throw new AgentRequestError("unavailable");
  }
  return body.items;
}

export async function fetchAgentAuthentication(
  baseUrl: string,
  access: string,
): Promise<AgentAuthentication> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/agent/authentication`,
    { headers: authHeaders(access) },
  );
  const body = await readJson(response);
  if (!response.ok || !isAgentAuthentication(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function requestAgentAuthentication(
  baseUrl: string,
  access: string,
): Promise<AgentAuthentication> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/agent/authentication`,
    {
      method: "POST",
      headers: { ...authHeaders(access), "Content-Type": "application/json" },
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isAgentAuthentication(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function fetchAgentModelSettings(
  baseUrl: string,
  access: string,
): Promise<AgentModelSettings> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}/v1/agent/models`, {
    headers: authHeaders(access),
  });
  const body = await readJson(response);
  if (!response.ok || !isAgentModelSettings(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function updateAgentModelSettings(
  baseUrl: string,
  access: string,
  modelId: string | null,
  reasoningEffort: string | null,
): Promise<AgentModelSettings> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}/v1/agent/models`, {
    method: "PUT",
    headers: { ...authHeaders(access), "Content-Type": "application/json" },
    body: JSON.stringify({ modelId, reasoningEffort }),
  });
  const body = await readJson(response);
  if (!response.ok || !isAgentModelSettings(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function createConversation(
  baseUrl: string,
  access: string,
  clientConversationId: string,
  title: string | null,
): Promise<Conversation> {
  return request<Conversation>(baseUrl, access, "/v1/conversations", {
    clientConversationId,
    title,
  });
}

export async function fetchConversationMessages(
  baseUrl: string,
  access: string,
  conversationId: string,
): Promise<ConversationMessage[]> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/conversations/${conversationId}/messages`,
    { headers: authHeaders(access) },
  );
  const body = await readJson(response);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isListResponse<ConversationMessage>(body)) {
    throw new AgentRequestError("unavailable");
  }
  return body.items;
}

export async function queueAgentTurn(
  baseUrl: string,
  access: string,
  conversationId: string,
  text: string,
  clientMessageId: string,
): Promise<QueuedAgentTurn> {
  return request<QueuedAgentTurn>(
    baseUrl,
    access,
    `/v1/conversations/${conversationId}/turns`,
    {
      clientMessageId,
      input: [{ type: "text", text }],
    },
  );
}

export async function fetchAgentJob(
  baseUrl: string,
  access: string,
  jobId: string,
): Promise<AgentJob> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/agent/jobs/${jobId}`,
    { headers: authHeaders(access) },
  );
  const body = await readJson(response);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isAgentJob(body)) throw new AgentRequestError("unavailable");
  return body;
}

export async function fetchLatestConversationJob(
  baseUrl: string,
  access: string,
  conversationId: string,
): Promise<AgentJob | undefined> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/conversations/${conversationId}/jobs/latest`,
    { headers: authHeaders(access) },
  );
  if (response.status === 204) return undefined;
  const body = await readJson(response);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isAgentJob(body)) throw new AgentRequestError("unavailable");
  return body;
}

export async function resolveAgentAction(
  baseUrl: string,
  access: string,
  jobId: string,
  decision: "approve" | "decline",
): Promise<AgentJob> {
  return request<AgentJob>(
    baseUrl,
    access,
    `/v1/agent/jobs/${jobId}/approval`,
    { decision },
  );
}

export async function streamConversationUpdates(
  baseUrl: string,
  access: string,
  conversationId: string,
  signal: AbortSignal,
  onSnapshot: (snapshot: ConversationStreamSnapshot) => void,
): Promise<void> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/conversations/${conversationId}/stream`,
    {
      headers: {
        ...authHeaders(access),
        Accept: "text/event-stream",
      },
      signal,
    },
  );
  if (!response.ok || !response.body) throw errorFromStatus(response.status);

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let pending = "";
  try {
    while (!signal.aborted) {
      const { done, value } = await reader.read();
      if (done) break;
      pending += decoder.decode(value, { stream: true });
      const frames = pending.split("\n\n");
      pending = frames.pop() ?? "";
      for (const frame of frames) {
        const snapshot = parseSnapshotFrame(frame);
        if (snapshot) onSnapshot(snapshot);
      }
    }
  } finally {
    reader.releaseLock();
  }
}

async function request<T>(
  baseUrl: string,
  access: string,
  path: string,
  body: unknown,
): Promise<T> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method: "POST",
    headers: { ...authHeaders(access), "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const payload = await readJson(response);
  if (!response.ok || !isRecord(payload)) {
    throw errorFromStatus(response.status);
  }
  return payload as T;
}

function authHeaders(access: string): HeadersInit {
  return { Accept: "application/json", Authorization: `Bearer ${access}` };
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/$/, "");
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

function errorFromStatus(status: number): AgentRequestError {
  if (status === 401) return new AgentRequestError("unauthorized");
  if (status === 404) return new AgentRequestError("notFound");
  if (status === 409) return new AgentRequestError("conflict");
  if (status >= 400 && status < 500) return new AgentRequestError("invalid");
  return new AgentRequestError("unavailable");
}

function isListResponse<T>(value: unknown): value is ListResponse<T> {
  return isRecord(value) && Array.isArray(value.items);
}

function isAgentJob(value: unknown): value is AgentJob {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.conversationId === "string" &&
    typeof value.state === "string" &&
    (value.pendingAction === null || isPendingAgentAction(value.pendingAction))
  );
}

function isConversationMessage(value: unknown): value is ConversationMessage {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.role === "string" &&
    typeof value.content === "string" &&
    (value.presentation === null ||
      isAssistantPresentation(value.presentation)) &&
    typeof value.status === "string" &&
    typeof value.createdAt === "string"
  );
}

function isAssistantPresentation(
  value: unknown,
): value is AssistantPresentation {
  return (
    isRecord(value) &&
    (value.kind === "summary" ||
      value.kind === "tasks" ||
      value.kind === "schedule" ||
      value.kind === "projects" ||
      value.kind === "composite") &&
    typeof value.title === "string" &&
    Array.isArray(value.items) &&
    value.items.every(isAssistantPresentationItem) &&
    (value.layout === "stack" ||
      value.layout === "split" ||
      value.layout === "focus") &&
    Array.isArray(value.sections) &&
    value.sections.every(isAssistantPresentationSection) &&
    (value.focusItemId === null || typeof value.focusItemId === "string")
  );
}

function isAssistantPresentationSection(value: unknown): boolean {
  return (
    isRecord(value) &&
    (value.kind === "tasks" ||
      value.kind === "schedule" ||
      value.kind === "projects") &&
    typeof value.title === "string" &&
    (value.view === "list" ||
      value.view === "checklist" ||
      value.view === "timeline" ||
      value.view === "cards") &&
    Array.isArray(value.itemIds) &&
    value.itemIds.every((itemId) => typeof itemId === "string")
  );
}

function isAssistantPresentationItem(
  value: unknown,
): value is AssistantPresentationItem {
  if (!isRecord(value) || typeof value.id !== "string") return false;
  if (value.type === "task") {
    return (
      (value.projectId === null || typeof value.projectId === "string") &&
      (value.projectTitle === null || typeof value.projectTitle === "string") &&
      typeof value.title === "string" &&
      typeof value.priority === "number" &&
      (value.dueAt === null || typeof value.dueAt === "string")
    );
  }
  if (value.type === "schedule") {
    return (
      typeof value.title === "string" &&
      typeof value.startsAt === "string" &&
      typeof value.endsAt === "string" &&
      typeof value.timeZone === "string"
    );
  }
  if (value.type === "project") {
    return (
      typeof value.workspaceId === "string" &&
      typeof value.title === "string" &&
      (value.objective === null || typeof value.objective === "string") &&
      (value.nextAction === null || typeof value.nextAction === "string") &&
      typeof value.riskLevel === "number" &&
      typeof value.openTaskCount === "number"
    );
  }
  return false;
}

function isPendingAgentAction(value: unknown): value is PendingAgentAction {
  return (
    isRecord(value) &&
    (value.kind === "create_task" || value.kind === "create_schedule") &&
    typeof value.title === "string" &&
    (value.startsAt === null || typeof value.startsAt === "string") &&
    (value.endsAt === null || typeof value.endsAt === "string")
  );
}

function parseSnapshotFrame(
  frame: string,
): ConversationStreamSnapshot | undefined {
  const event = frame
    .split("\n")
    .find((line) => line.startsWith("event:"))
    ?.slice("event:".length)
    .trim();
  if (event !== "snapshot") return undefined;
  const data = frame
    .split("\n")
    .filter((line) => line.startsWith("data:"))
    .map((line) => line.slice("data:".length).trimStart())
    .join("\n");
  if (!data) return undefined;
  try {
    const value: unknown = JSON.parse(data);
    if (
      !isRecord(value) ||
      !Array.isArray(value.messages) ||
      !value.messages.every(isConversationMessage) ||
      !(value.job === null || isAgentJob(value.job))
    ) {
      return undefined;
    }
    return {
      messages: value.messages,
      job: value.job,
    };
  } catch {
    return undefined;
  }
}

function isAgentAuthentication(value: unknown): value is AgentAuthentication {
  return (
    isRecord(value) &&
    typeof value.state === "string" &&
    (value.verificationUrl === null ||
      typeof value.verificationUrl === "string") &&
    (value.userCode === null || typeof value.userCode === "string")
  );
}

function isAgentModelSettings(value: unknown): value is AgentModelSettings {
  return (
    isRecord(value) &&
    Array.isArray(value.items) &&
    value.items.every(isAgentModel) &&
    (value.selectedModelId === null ||
      typeof value.selectedModelId === "string") &&
    (value.selectedReasoningEffort === null ||
      typeof value.selectedReasoningEffort === "string")
  );
}

function isAgentModel(value: unknown): value is AgentModel {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.displayName === "string" &&
    typeof value.description === "string" &&
    typeof value.isDefault === "boolean" &&
    typeof value.defaultReasoningEffort === "string" &&
    Array.isArray(value.supportedReasoningEfforts) &&
    value.supportedReasoningEfforts.every(isAgentReasoningEffort)
  );
}

function isAgentReasoningEffort(value: unknown): value is AgentReasoningEffort {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.description === "string"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
