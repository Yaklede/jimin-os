import { createUuidV7, isUuidV7 } from "../uuid";

export type VoiceCommandResult = {
  kind:
    | "schedule_listed"
    | "schedule_created"
    | "tasks_listed"
    | "task_created"
    | "needs_details"
    | "continue_conversation";
  message: string;
  destination: "home" | "calendar" | "conversation";
  items: VoiceCommandResultItem[];
};

export type VoiceCommandResultItem = {
  itemType: "task" | "schedule";
  id: string;
  title: string;
  dueAt: string | null;
  startsAt: string | null;
  endsAt: string | null;
  priority: number | null;
};

export class VoiceCommandRequestError extends Error {
  readonly code: "unauthorized" | "invalid" | "unavailable";

  constructor(code: VoiceCommandRequestError["code"]) {
    super(code);
    this.name = "VoiceCommandRequestError";
    this.code = code;
  }
}

export async function processVoiceCommand(
  baseUrl: string,
  access: string,
  text: string,
  clientMutationId = createUuidV7(),
): Promise<VoiceCommandResult> {
  if (!isUuidV7(clientMutationId)) {
    throw new VoiceCommandRequestError("invalid");
  }
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/assistant/voice-commands`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        clientMutationId,
        text,
        referenceAt: localReferenceAt(new Date()),
        timeZone:
          Intl.DateTimeFormat().resolvedOptions().timeZone || "Asia/Seoul",
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok) throw errorFromStatus(response.status);
  if (!isVoiceCommandResult(body)) {
    throw new VoiceCommandRequestError("unavailable");
  }
  return body;
}

function localReferenceAt(date: Date): string {
  const offsetMinutes = -date.getTimezoneOffset();
  const localTime = new Date(date.getTime() + offsetMinutes * 60_000)
    .toISOString()
    .slice(0, -1);
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const absoluteOffset = Math.abs(offsetMinutes);
  const hours = String(Math.floor(absoluteOffset / 60)).padStart(2, "0");
  const minutes = String(absoluteOffset % 60).padStart(2, "0");
  return `${localTime}${sign}${hours}:${minutes}`;
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

function errorFromStatus(status: number): VoiceCommandRequestError {
  if (status === 401) return new VoiceCommandRequestError("unauthorized");
  if (status >= 400 && status < 500) {
    return new VoiceCommandRequestError("invalid");
  }
  return new VoiceCommandRequestError("unavailable");
}

function isVoiceCommandResult(value: unknown): value is VoiceCommandResult {
  if (!isRecord(value)) return false;
  return (
    isVoiceCommandKind(value.kind) &&
    typeof value.message === "string" &&
    Array.isArray(value.items) &&
    value.items.every(isVoiceCommandResultItem) &&
    (value.destination === "home" ||
      value.destination === "calendar" ||
      value.destination === "conversation")
  );
}

function isVoiceCommandResultItem(
  value: unknown,
): value is VoiceCommandResultItem {
  if (!isRecord(value)) return false;
  return (
    (value.itemType === "task" || value.itemType === "schedule") &&
    typeof value.id === "string" &&
    typeof value.title === "string" &&
    (value.dueAt === null || typeof value.dueAt === "string") &&
    (value.startsAt === null || typeof value.startsAt === "string") &&
    (value.endsAt === null || typeof value.endsAt === "string") &&
    (value.priority === null || typeof value.priority === "number")
  );
}

function isVoiceCommandKind(
  value: unknown,
): value is VoiceCommandResult["kind"] {
  return (
    value === "schedule_listed" ||
    value === "schedule_created" ||
    value === "tasks_listed" ||
    value === "task_created" ||
    value === "needs_details" ||
    value === "continue_conversation"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
