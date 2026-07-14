import { describe, expect, it } from "vitest";

import { conversationIdForRequest } from "./conversationRouting";

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
