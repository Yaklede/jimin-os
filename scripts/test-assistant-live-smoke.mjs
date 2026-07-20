import { randomBytes } from "node:crypto";

const configuredBaseUrl =
  process.env.JIMIN_LIVE_SMOKE_BASE_URL ?? "http://127.0.0.1:8080";
const baseUrl = new URL(configuredBaseUrl);
if (
  baseUrl.protocol !== "http:" ||
  baseUrl.hostname !== "127.0.0.1" ||
  baseUrl.pathname !== "/" ||
  baseUrl.search ||
  baseUrl.hash
) {
  throw new Error(
    "Live assistant smoke tests are restricted to the loopback test server.",
  );
}

const session = await request("/v1/access/session", {
  method: "POST",
  body: {
    installationId: uuidV7(),
    platform: "macos",
    name: "Assistant live smoke",
    appVersion: "contract-smoke",
    osVersion: process.platform,
  },
});
const accessToken = requiredString(session.accessToken, "session access token");
const authentication = await request("/v1/agent/authentication", {
  accessToken,
});
if (authentication.state !== "ready") {
  throw new Error(`Local agent is not ready: ${String(authentication.state)}`);
}
const models = await request("/v1/agent/models", { accessToken });
if (!Array.isArray(models.items) || models.items.length === 0) {
  throw new Error("Local agent returned no processing models.");
}

const conversation = await request("/v1/conversations", {
  method: "POST",
  accessToken,
  body: {
    clientConversationId: uuidV7(),
    title: "[SMOKE] 일반 대화",
  },
});
const conversationId = requiredString(conversation.id, "conversation ID");
await request(`/v1/conversations/${conversationId}/turns`, {
  method: "POST",
  accessToken,
  body: {
    clientMessageId: uuidV7(),
    input: [
      {
        type: "text",
        text: "아 일하기 싫다. 일반 대화로 짧게 답해줘.",
      },
    ],
  },
});

const stream = await fetch(
  new URL(`/v1/conversations/${conversationId}/stream`, baseUrl),
  {
    headers: {
      Accept: "text/event-stream",
      Authorization: `Bearer ${accessToken}`,
    },
    signal: AbortSignal.timeout(90_000),
  },
);
if (!stream.ok || !stream.body) {
  throw new Error(`Conversation stream failed with HTTP ${stream.status}.`);
}

let snapshots = 0;
let sawInProgress = false;
let terminalState;
let pending = "";
const reader = stream.body.getReader();
const decoder = new TextDecoder();
while (terminalState === undefined) {
  const { done, value } = await reader.read();
  if (done) break;
  pending += decoder.decode(value, { stream: true });
  const frames = pending.split("\n\n");
  pending = frames.pop() ?? "";
  for (const frame of frames) {
    const dataLine = frame.split("\n").find((line) => line.startsWith("data:"));
    if (!dataLine) continue;
    const snapshot = JSON.parse(dataLine.slice(5).trim());
    snapshots += 1;
    const state = snapshot.job?.state;
    if (["queued", "claimed", "running", "retry_wait"].includes(state)) {
      sawInProgress = true;
    }
    if (["completed", "failed", "cancelled", "declined"].includes(state)) {
      terminalState = state;
    }
  }
}
reader.releaseLock();

if (terminalState !== "completed") {
  throw new Error(
    `General conversation ended in ${terminalState ?? "no state"}.`,
  );
}
const messages = await request(`/v1/conversations/${conversationId}/messages`, {
  accessToken,
});
const assistantMessage = Array.isArray(messages.items)
  ? messages.items.find(
      (message) =>
        message.role === "assistant" && message.status === "completed",
    )
  : undefined;
if (
  !assistantMessage ||
  typeof assistantMessage.content !== "string" ||
  assistantMessage.content.trim().length === 0
) {
  throw new Error("General conversation did not persist a completed answer.");
}
if (assistantMessage.presentation !== null) {
  throw new Error("General conversation unexpectedly produced a work canvas.");
}
if (snapshots < 2 || !sawInProgress) {
  throw new Error(
    "Conversation did not expose observable in-progress streaming state.",
  );
}

const taskMarker = `JOS-SMOKE-${uuidV7().slice(0, 8).toUpperCase()}`;
await request(`/v1/conversations/${conversationId}/turns`, {
  method: "POST",
  accessToken,
  body: {
    clientMessageId: uuidV7(),
    input: [
      {
        type: "text",
        text: `내일 할 일에 ${taskMarker} 계약서 검토 추가해줘. 계약서의 누락된 조항을 확인하면 완료야.`,
      },
    ],
  },
});
const mutationStream = await waitForTerminalConversation(
  conversationId,
  accessToken,
);
if (mutationStream.terminalState !== "completed") {
  throw new Error(
    `Task creation ended in ${mutationStream.terminalState ?? "no state"}.`,
  );
}
const taskList = await request("/v1/tasks", { accessToken });
const createdTask = Array.isArray(taskList.items)
  ? taskList.items.find(
      (task) =>
        typeof task.title === "string" && task.title.includes(taskMarker),
    )
  : undefined;
if (!createdTask) {
  throw new Error("Natural-language task creation did not persist its marker.");
}
try {
  if (typeof createdTask.notes !== "string" || !createdTask.notes.trim()) {
    throw new Error("Assistant-created task did not include a useful brief.");
  }
  if (koreaDate(createdTask.dueAt) !== koreaDate(Date.now() + 86_400_000)) {
    throw new Error("Assistant-created task was not assigned to tomorrow.");
  }
} finally {
  await request(`/v1/tasks/${createdTask.id}`, {
    method: "DELETE",
    accessToken,
    body: { expectedVersion: createdTask.version },
  });
}

console.log(
  `Assistant live smoke passed: ${snapshots + mutationStream.snapshots} stream snapshots, general conversation and task creation.`,
);

async function waitForTerminalConversation(conversationId, accessToken) {
  const response = await fetch(
    new URL(`/v1/conversations/${conversationId}/stream`, baseUrl),
    {
      headers: {
        Accept: "text/event-stream",
        Authorization: `Bearer ${accessToken}`,
      },
      signal: AbortSignal.timeout(90_000),
    },
  );
  if (!response.ok || !response.body) {
    throw new Error(`Conversation stream failed with HTTP ${response.status}.`);
  }
  let streamSnapshots = 0;
  let terminalState;
  let streamPending = "";
  const streamReader = response.body.getReader();
  const streamDecoder = new TextDecoder();
  while (terminalState === undefined) {
    const { done, value } = await streamReader.read();
    if (done) break;
    streamPending += streamDecoder.decode(value, { stream: true });
    const frames = streamPending.split("\n\n");
    streamPending = frames.pop() ?? "";
    for (const frame of frames) {
      const dataLine = frame
        .split("\n")
        .find((line) => line.startsWith("data:"));
      if (!dataLine) continue;
      const snapshot = JSON.parse(dataLine.slice(5).trim());
      streamSnapshots += 1;
      const state = snapshot.job?.state;
      if (["completed", "failed", "cancelled", "declined"].includes(state)) {
        terminalState = state;
      }
    }
  }
  streamReader.releaseLock();
  return { snapshots: streamSnapshots, terminalState };
}

async function request(path, options = {}) {
  const headers = { Accept: "application/json" };
  if (options.accessToken) {
    headers.Authorization = `Bearer ${options.accessToken}`;
  }
  if (options.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }
  const response = await fetch(new URL(path, baseUrl), {
    method: options.method ?? "GET",
    headers,
    body: options.body === undefined ? undefined : JSON.stringify(options.body),
  });
  const body = await response.json().catch(() => undefined);
  if (!response.ok) {
    const code = body?.error?.code;
    throw new Error(
      `${path} failed with HTTP ${response.status}${code ? ` (${code})` : ""}.`,
    );
  }
  return body;
}

function requiredString(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    throw new Error(`Missing ${label}.`);
  }
  return value;
}

function koreaDate(value) {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.valueOf())) return undefined;
  return new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  }).format(date);
}

function uuidV7() {
  const bytes = randomBytes(16);
  let timestamp = BigInt(Date.now());
  for (let index = 5; index >= 0; index -= 1) {
    bytes[index] = Number(timestamp & 0xffn);
    timestamp >>= 8n;
  }
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
