import { PlanningRequestError } from "./planning";

export type MeetingStatus =
  | "recording"
  | "transcribing"
  | "queued"
  | "analyzing"
  | "review_ready"
  | "applied"
  | "failed";

export type MeetingRecordingState =
  | "recording"
  | "queued"
  | "claimed"
  | "running"
  | "completed"
  | "failed"
  | "cancelled";

export type MeetingActionStatus = "suggested" | "applied" | "rejected";

export interface Meeting {
  id: string;
  workspaceId: string | null;
  projectId: string | null;
  projectTitle: string | null;
  title: string;
  purpose: string | null;
  participants: string[];
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
  assigneeName: string | null;
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
  recording: MeetingRecording | null;
  speakers: MeetingSpeaker[];
  transcriptSegments: MeetingTranscriptSegment[];
  decisions: MeetingDecision[];
  actionItems: MeetingActionItem[];
}

export interface MeetingRecording {
  id: string;
  meetingId: string;
  state: MeetingRecordingState;
  mimeType: string | null;
  notes: string;
  durationMilliseconds: number | null;
  chunkCount: number;
  byteLength: number;
  errorCode: string | null;
  startedAt: string;
  finalizedAt: string | null;
  finishedAt: string | null;
  updatedAt: string;
  version: number;
}

export interface MeetingSpeaker {
  id: string;
  meetingId: string;
  speakerKey: string;
  displayName: string | null;
  ordinal: number;
}

export interface MeetingTranscriptSegment {
  id: string;
  meetingId: string;
  speakerId: string;
  speakerKey: string;
  speakerName: string | null;
  ordinal: number;
  startsAtMilliseconds: number;
  endsAtMilliseconds: number;
  text: string;
  confidence: number | null;
  isFinal: boolean;
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
    purpose?: string;
    participants?: string[];
    transcript: string;
    workspaceId?: string;
    projectId?: string;
    startedAt?: string;
    durationSeconds?: number;
  },
): Promise<Meeting> {
  return request<Meeting>(baseUrl, access, "/v1/meetings", {
    title: input.title,
    purpose: input.purpose ?? null,
    participants: input.participants ?? [],
    transcript: input.transcript,
    workspaceId: input.workspaceId ?? null,
    projectId: input.projectId ?? null,
    startedAt: input.startedAt ?? null,
    durationSeconds: input.durationSeconds ?? null,
  });
}

export async function startMeetingRecording(
  baseUrl: string,
  access: string,
  input: {
    title: string;
    purpose?: string;
    participants?: string[];
    workspaceId?: string;
    projectId?: string;
    startedAt: string;
  },
): Promise<{ meeting: Meeting; recording: MeetingRecording }> {
  return request(baseUrl, access, "/v1/meeting-recordings", {
    title: input.title,
    purpose: input.purpose ?? null,
    participants: input.participants ?? [],
    workspaceId: input.workspaceId ?? null,
    projectId: input.projectId ?? null,
    startedAt: input.startedAt,
  });
}

export async function uploadMeetingRecordingChunk(
  baseUrl: string,
  access: string,
  recordingId: string,
  sequence: number,
  blob: Blob,
  mimeType: string,
): Promise<MeetingRecording> {
  return request(
    baseUrl,
    access,
    `/v1/meeting-recordings/${encodeURIComponent(recordingId)}/chunks/${sequence}`,
    {
      mimeType,
      audioBase64: await blobAsBase64(blob),
    },
    "PUT",
  );
}

export async function updateMeetingRecordingNotes(
  baseUrl: string,
  access: string,
  recordingId: string,
  notes: string,
): Promise<MeetingRecording> {
  return request(
    baseUrl,
    access,
    `/v1/meeting-recordings/${encodeURIComponent(recordingId)}/notes`,
    { notes },
    "PUT",
  );
}

export async function finalizeMeetingRecording(
  baseUrl: string,
  access: string,
  recordingId: string,
  input: { mimeType: string; durationMilliseconds: number },
): Promise<MeetingRecording> {
  return request(
    baseUrl,
    access,
    `/v1/meeting-recordings/${encodeURIComponent(recordingId)}/finalize`,
    input,
  );
}

export async function cancelMeetingRecording(
  baseUrl: string,
  access: string,
  recordingId: string,
): Promise<void> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/meeting-recordings/${encodeURIComponent(recordingId)}/cancel`,
    {
      method: "POST",
      headers: authHeaders(access),
    },
  );
  if (!response.ok) throw errorFrom(response.status);
}

export async function updateMeetingAction(
  baseUrl: string,
  access: string,
  meetingId: string,
  item: MeetingActionItem,
  input: {
    title: string;
    notes?: string;
    assigneeName?: string;
    priority: number;
    dueAt?: string;
    startsAt?: string;
    endsAt?: string;
    timeZone?: string;
  },
): Promise<MeetingActionItem> {
  return request<MeetingActionItem>(
    baseUrl,
    access,
    `/v1/meetings/${encodeURIComponent(meetingId)}/action-items/${encodeURIComponent(item.id)}`,
    {
      expectedVersion: item.version,
      title: input.title,
      notes: input.notes ?? null,
      assigneeName: input.assigneeName ?? null,
      priority: input.priority,
      dueAt: input.dueAt ?? null,
      startsAt: input.startsAt ?? null,
      endsAt: input.endsAt ?? null,
      timeZone: input.timeZone ?? null,
    },
    "PUT",
  );
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
  method: "POST" | "PUT" = "POST",
): Promise<T> {
  const response = await fetch(`${normalizeBaseUrl(baseUrl)}${path}`, {
    method,
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

async function blobAsBase64(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  const batchSize = 32_768;
  for (let offset = 0; offset < bytes.length; offset += batchSize) {
    binary += String.fromCharCode(
      ...bytes.subarray(offset, offset + batchSize),
    );
  }
  return btoa(binary);
}
