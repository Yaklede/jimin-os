import { expect, it, vi } from "vitest";

import { AgentRequestError } from "../api/agent";
import { deleteSelectedConversations } from "./conversationBulkDelete";

it("deletes only the confirmed IDs, deduplicates them, and leaves failures for retry", async () => {
  const remove = vi.fn(async (id: string) => {
    if (id === "busy") throw new AgentRequestError("conflict");
    if (id === "offline") throw new AgentRequestError("unavailable");
  });

  expect(
    await deleteSelectedConversations(
      ["done", "busy", "offline", "done", "also-done"],
      remove,
    ),
  ).toEqual({
    deleted: ["done", "also-done"],
    failed: ["busy", "offline"],
    busy: 1,
  });
  expect(remove.mock.calls).toEqual([
    ["done"],
    ["busy"],
    ["offline"],
    ["also-done"],
  ]);
});

it("bounds the number of deletion requests in flight", async () => {
  const release: Array<() => void> = [];
  const remove = vi.fn(
    () => new Promise<void>((resolve) => release.push(resolve)),
  );
  const deletion = deleteSelectedConversations(
    ["a", "b", "c", "d", "e"],
    remove,
  );
  expect(remove).toHaveBeenCalledTimes(4);
  for (const resolve of release.splice(0)) resolve();
  await vi.waitFor(() => expect(remove).toHaveBeenCalledTimes(5));
  release[0]();
  expect((await deletion).deleted).toEqual(["a", "b", "c", "d", "e"]);
});
