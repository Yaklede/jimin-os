import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { expect, it } from "vitest";

import { ConversationWorkspace } from "./ConversationWorkspace";

it("renders the dedicated conversation list without a composer or previous transcript", () => {
  const markup = renderToStaticMarkup(
    createElement(ConversationWorkspace, {
      historyOnly: true,
      conversations: [
        {
          id: "old-chat",
          title: "남겨둔 대화",
          surface: "chat",
          status: "active",
          lastMessageAt: "2026-11-23T06:00:00Z",
          version: 1,
        },
        {
          id: "unused-draft",
          title: null,
          surface: "home",
          status: "active",
          lastMessageAt: null,
          version: 1,
        },
      ],
      historyJobs: {
        "old-chat": {
          id: "old-job",
          conversationId: "old-chat",
          state: "waiting_approval",
          createdAt: "2026-11-23T06:00:00Z",
          finishedAt: null,
          version: 1,
          pendingAction: null,
        },
      },
      messages: [],
      selectedConversationId: undefined,
      job: undefined,
      hasActiveJob: false,
      authentication: undefined,
      authenticationRequesting: false,
      loading: false,
      error: undefined,
      initialDraft: undefined,
      onSelect() {},
      onReturnHome() {},
      onInitialDraftApplied() {},
      onStartConversation: async () => true,
      onDeleteConversation: async () => {},
      onStartAuthentication: async () => {},
      onSend: async () => true,
      onResolveAction: async () => {},
    }),
  );
  expect(markup).toContain('data-view="history"');
  expect(markup).toContain(">대화 목록</h1>");
  expect(markup).toContain('aria-expanded="false">선택</button>');
  expect(markup).toContain("남겨둔 대화");
  expect(markup).toContain("승인이 필요해요.");
  expect(markup).not.toContain("이름 없는 대화");
  expect(markup).not.toContain("assistant-conversation");
  expect(markup).not.toContain("agent-message");
});
