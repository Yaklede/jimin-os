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
  status: "pending" | "streaming" | "completed" | "failed" | "cancelled";
  createdAt: string;
  completedAt: string | null;
  version: number;
}

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
}

export interface QueuedAgentTurn {
  jobId: string;
  messageId: string;
  conversationId: string;
  state: AgentJob["state"];
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

export async function createConversation(
  baseUrl: string,
  access: string,
  title: string | null,
): Promise<Conversation> {
  return request<Conversation>(baseUrl, access, "/v1/conversations", {
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
): Promise<QueuedAgentTurn> {
  return request<QueuedAgentTurn>(
    baseUrl,
    access,
    `/v1/conversations/${conversationId}/turns`,
    {
      clientMessageId: crypto.randomUUID(),
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
    typeof value.state === "string"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
