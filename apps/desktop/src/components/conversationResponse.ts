import { type AgentJob, type ConversationMessage } from "../api/agent";
import { copy } from "../copy";

export function assistantTurnFeedback(
  job: AgentJob | undefined,
  reply: ConversationMessage | undefined,
):
  | {
      tone: "busy" | "success" | "warning" | "error" | "neutral";
      label: string;
    }
  | undefined {
  const c = copy.conversations;
  switch (job?.state) {
    case "queued":
    case "claimed":
      return { tone: "busy", label: c.preparing };
    case "running":
      return { tone: "busy", label: c.streaming };
    case "retry_wait":
      return { tone: "busy", label: c.retryPending };
    case "waiting_approval":
      return { tone: "warning", label: c.approvalNeeded };
    case "completed":
      return { tone: "success", label: c.replyReady };
    case "failed":
      return {
        tone: "error",
        label:
          reply?.status === "completed" && reply.content.trim()
            ? c.responseCheck
            : reply?.content.trim()
              ? c.responseInterrupted
              : c.failed,
      };
    case "cancelled":
      return { tone: "neutral", label: c.turnCancelled };
    case "declined":
      return { tone: "neutral", label: c.turnDeclined };
    default:
      return undefined;
  }
}

export function assistantResponseAfterLatestRequest(
  messages: ConversationMessage[],
): ConversationMessage | undefined {
  let latestUserIndex = -1;
  for (let index = messages.length - 1; index >= 0; index -= 1) {
    if (messages[index]?.role === "user") {
      latestUserIndex = index;
      break;
    }
  }
  if (latestUserIndex < 0) return undefined;
  for (let index = messages.length - 1; index > latestUserIndex; index -= 1) {
    const message = messages[index];
    if (message?.role === "assistant") return message;
  }
  return undefined;
}
