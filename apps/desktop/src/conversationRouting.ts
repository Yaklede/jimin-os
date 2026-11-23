import type { Conversation } from "./api/agent";

export type ConversationSendOptions = {
  startFresh?: boolean;
  targetConversationId?: string;
  rememberForHome?: boolean;
};

export function freshHomeRequest(): ConversationSendOptions {
  return { startFresh: true, rememberForHome: true };
}

export function conversationIdForRequest(
  selectedConversationId: string | undefined,
  options: ConversationSendOptions,
): string | undefined {
  return (
    options.targetConversationId ??
    (options.startFresh ? undefined : selectedConversationId)
  );
}

export function mergeCreatedConversation(
  conversations: Conversation[],
  created: Conversation,
): Conversation[] {
  return [
    created,
    ...conversations.flatMap((known): Conversation[] => {
      if (known.id === created.id) return [];
      if (created.surface !== "home" || known.surface !== "home")
        return [known];
      return known.lastMessageAt || known.title
        ? [{ ...known, surface: "chat" }]
        : [];
    }),
  ];
}
