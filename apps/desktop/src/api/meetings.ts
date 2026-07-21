import { PlanningRequestError } from "./planning";

export type MeetingStatus =
  "queued" | "analyzing" | "review_ready" | "applied" | "failed";

export type MeetingActionStatus = "suggested" | "applied" | "rejected";

export interface Meeting {
  id: string;
  workspaceId: string | null;
  projectId: string | null;
  projectTitle: string | null;
  title: string;
  transcript: string;
  startedAt: string | null;
  durationSeconds: number | null;
  status: MeetingStatus;
  summary: string | null;
  topics: string[];
  risks: string[];
  followUp: string | null;
  analyzedAt: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
}

export type MeetingSummary = Omit<Meeting, "transcript">;

export interface MeetingDecision {
  id: string;
  content: string;
  rationale: string | null;
  sourceExcerpt: string;
  sourceTimestampSeconds: number | null;
}

export interface MeetingActionItem {
  id: string;
  meetingId: string;
  kind: "task" | "schedule";
  projectId: string | null;
  title: string;
  notes: string | null;
  priority: number;
  dueAt: string | null;
  startsAt: string | null;
  endsAt: string | null;
  timeZone: string | null;
  sourceExcerpt: string;
  confidence: number;
  status: MeetingActionStatus;
  targetEntityId: string;
  version: number;
}

export interface MeetingDetail extends Meeting {
  decisions: MeetingDecision[];
  actionItems: MeetingActionItem[];
}

export async function fetchMeetings(
  baseUrl: string,
  access: string,
): Promise<MeetingSummary[]> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}/v1/meetings`, {
    headers: authHeaders(access),
  });
  const body = await readJson(response);
  if (!response.ok || !isMeetingList(body)) throw errorFrom(response.status);
  return body.items;
}

export async function reanalyzeMeeting(
  baseUrl: string,
  access: string,
  meetingId: string,
): Promise<Meeting> {
  return request<Meeting>(
    baseUrl,
    access,
    `/v1/meetings/${encodeURIComponent(meetingId)}/reanalyze`,
    {},
  );
}

export async function fetchMeeting(
  baseUrl: string,
  access: string,
  meetingId: string,
): Promise<MeetingDetail> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/meetings/${encodeURIComponent(meetingId)}`,
    { headers: authHeaders(access) },
  );
  const body = await readJson(response);
  if (!response.ok || !isRecord(body)) throw errorFrom(response.status);
  return body as unknown as MeetingDetail;
}

export async function createMeeting(
  baseUrl: string,
  access: string,
  input: {
    title: string;
    transcript: string;
    workspaceId?: string;
    projectId?: string;
    startedAt?: string;
    durationSeconds?: number;
  },
): Promise<Meeting> {
  return request<Meeting>(baseUrl, access, "/v1/meetings", {
    title: input.title,
    transcript: input.transcript,
    workspaceId: input.workspaceId ?? null,
    projectId: input.projectId ?? null,
    startedAt: input.startedAt ?? null,
    durationSeconds: input.durationSeconds ?? null,
  });
}

export async function decideMeetingAction(
  baseUrl: string,
  access: string,
  meetingId: string,
  itemId: string,
  decision: "approve" | "reject",
): Promise<MeetingActionItem> {
  return request<MeetingActionItem>(
    baseUrl,
    access,
    `/v1/meetings/${encodeURIComponent(meetingId)}/action-items/${encodeURIComponent(itemId)}/decisions`,
    { decision },
  );
}

async function request<T>(
  baseUrl: string,
  access: string,
  path: string,
  body: unknown,
): Promise<T> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method: "POST",
    headers: {
      ...authHeaders(access),
      "Content-Type": "application/json",
    },
    body: JSON.stringify(body),
  });
  const payload = await readJson(response);
  if (!response.ok || !isRecord(payload)) throw errorFrom(response.status);
  return payload as T;
}

function authHeaders(access: string): Record<string, string> {
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

function errorFrom(status: number): PlanningRequestError {
  if (status === 401) return new PlanningRequestError("unauthorized");
  if (status === 409) return new PlanningRequestError("conflict");
  if (status >= 400 && status < 500) {
    return new PlanningRequestError("invalid");
  }
  return new PlanningRequestError("unavailable");
}

function isMeetingList(value: unknown): value is { items: MeetingSummary[] } {
  return isRecord(value) && Array.isArray(value.items);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
