import { describe, expect, it } from "vitest";

import { type AgentJob, type ConversationMessage } from "../api/agent";
import { copy } from "../copy";
import {
  assistantResponseAfterLatestRequest,
  hasDisplayableAssistantResponse,
  isAssistantMessageStreaming,
} from "./ConversationWorkspace";
import { commandStatus } from "./HomeWorkspace";
import { assistantTurnFeedback } from "./conversationResponse";

function message(
  id: string,
  role: ConversationMessage["role"],
  content: string,
  status: ConversationMessage["status"] = "completed",
): ConversationMessage {
  return {
    id,
    role,
    content,
    presentation: null,
    status,
    createdAt: "2026-07-21T05:55:00Z",
    completedAt: status === "streaming" ? null : "2026-07-21T05:55:01Z",
    version: 1,
  };
}

function job(state: AgentJob["state"]): AgentJob {
  return {
    id: "019b0000-0000-7000-8000-000000000001",
    conversationId: "019b0000-0000-7000-8000-000000000002",
    state,
    createdAt: "2026-07-21T05:55:00Z",
    finishedAt: ["completed", "failed", "cancelled", "declined"].includes(state)
      ? "2026-07-21T05:55:01Z"
      : null,
    version: 1,
    pendingAction: null,
  };
}

describe("assistant response terminal state", () => {
  it("shows durable request stages without treating drafting or approval as completion", () => {
    expect(assistantTurnFeedback(job("queued"), undefined)?.label).toBe(
      copy.conversations.preparing,
    );
    expect(assistantTurnFeedback(job("running"), undefined)?.label).toBe(
      copy.conversations.streaming,
    );
    expect(
      assistantTurnFeedback(job("waiting_approval"), undefined)?.tone,
    ).toBe("warning");
    expect(assistantTurnFeedback(job("completed"), undefined)?.label).toBe(
      copy.conversations.replyReady,
    );
  });

  it("keeps an interrupted response distinct from confirmed completion", () => {
    const partial = message(
      "partial",
      "assistant",
      "먼저 확인한 내용",
      "streaming",
    );
    expect(assistantTurnFeedback(job("failed"), partial)).toEqual({
      tone: "error",
      label: copy.conversations.responseInterrupted,
    });
    expect(assistantTurnFeedback(job("cancelled"), partial)?.tone).toBe(
      "neutral",
    );
  });

  it("selects only an assistant response created after the latest request", () => {
    const oldResponse = message("assistant-old", "assistant", "이전 답변");
    const latestRequest = message("user-new", "user", "내일 미팅 추가");

    expect(
      assistantResponseAfterLatestRequest([oldResponse, latestRequest]),
    ).toBeUndefined();

    const clarification = message(
      "assistant-new",
      "assistant",
      "시작 시간과 종료 시간을 알려주세요.",
      "streaming",
    );
    expect(
      assistantResponseAfterLatestRequest([
        oldResponse,
        latestRequest,
        clarification,
      ]),
    ).toEqual(clarification);
  });

  it("stops the streaming indicator when the job is terminal", () => {
    const clarification = message(
      "assistant-new",
      "assistant",
      "시작 시간과 종료 시간을 알려주세요.",
      "streaming",
    );

    expect(isAssistantMessageStreaming(clarification, job("running"))).toBe(
      true,
    );
    expect(isAssistantMessageStreaming(clarification, job("failed"))).toBe(
      false,
    );
  });

  it("shows a received clarification instead of a generic failure", () => {
    const clarification = message(
      "assistant-new",
      "assistant",
      "시작 시간과 종료 시간을 알려주세요.",
      "failed",
    );

    expect(hasDisplayableAssistantResponse(clarification)).toBe(true);
    expect(commandStatus(job("failed"), clarification)).toEqual({
      title: copy.home.commandResponseReceived,
      description: clarification.content,
      needsReview: true,
    });
  });
});
