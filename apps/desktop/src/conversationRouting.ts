export type ConversationSendOptions = {
  startFresh?: boolean;
  targetConversationId?: string;
  rememberForHome?: boolean;
};

export function conversationIdForRequest(
  selectedConversationId: string | undefined,
  options: ConversationSendOptions,
): string | undefined {
  return (
    options.targetConversationId ??
    (options.startFresh ? undefined : selectedConversationId)
  );
}
