import { AgentRequestError } from "../api/agent";

export type ConversationBulkDeleteResult = {
  deleted: string[];
  failed: string[];
  busy: number;
};

const DELETE_CONCURRENCY = 4;

export async function deleteSelectedConversations(
  conversationIds: readonly string[],
  remove: (id: string) => Promise<void>,
): Promise<ConversationBulkDeleteResult> {
  const ids = [...new Set(conversationIds)];
  const result: ConversationBulkDeleteResult = {
    deleted: [],
    failed: [],
    busy: 0,
  };
  for (let start = 0; start < ids.length; start += DELETE_CONCURRENCY) {
    const batch = ids.slice(start, start + DELETE_CONCURRENCY);
    const outcomes = await Promise.allSettled(
      batch.map(async (id) => remove(id)),
    );
    outcomes.forEach((outcome, index) => {
      if (outcome.status === "fulfilled") {
        result.deleted.push(batch[index]);
      } else {
        result.failed.push(batch[index]);
        if (
          outcome.reason instanceof AgentRequestError &&
          outcome.reason.code === "conflict"
        ) {
          result.busy += 1;
        }
      }
    });
  }
  return result;
}
