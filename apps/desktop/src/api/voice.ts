export type VoiceCommandResult = {
  kind:
    | "schedule_listed"
    | "schedule_created"
    | "tasks_listed"
    | "task_created"
    | "needs_details"
    | "continue_conversation";
  message: string;
  destination: "calendar" | "conversation";
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
): Promise<VoiceCommandResult> {
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
        text,
        referenceAt: new Date().toISOString(),
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
    (value.destination === "calendar" || value.destination === "conversation")
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
