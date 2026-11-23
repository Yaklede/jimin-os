import { describe, expect, it } from "vitest";

import type { Conversation } from "./api/agent";
import {
  conversationIdForRequest,
  mergeCreatedConversation,
} from "./conversationRouting";

it("keeps a previous home thread in history but does not pile up empty drafts", () => {
  const draft: Conversation = {
    id: "empty",
    surface: "home",
    status: "active",
    title: null,
    lastMessageAt: null,
    version: 1,
  };
  const old = { ...draft, id: "previous", title: "이전 질문" };
  const merged = mergeCreatedConversation([old], draft);
  expect(merged).toEqual([draft, { ...old, surface: "chat" }]);
  const replacement = { ...draft, id: "new" };
  expect(mergeCreatedConversation(merged, replacement)).toEqual([
    replacement,
    { ...old, surface: "chat" },
  ]);
});

describe("conversationIdForRequest", () => {
  it("starts a fresh home conversation instead of reusing an unrelated thread", () => {
    expect(
      conversationIdForRequest("selected-chat", { startFresh: true }),
    ).toBeUndefined();
  });

  it("continues the remembered home conversation even after another thread was selected", () => {
    expect(
      conversationIdForRequest("selected-chat", {
        targetConversationId: "home-conversation",
      }),
    ).toBe("home-conversation");
  });

  it("keeps the selected conversation for the full chat surface", () => {
    expect(conversationIdForRequest("selected-chat", {})).toBe("selected-chat");
  });
});
