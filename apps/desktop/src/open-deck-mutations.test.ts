import { describe, it, expect } from "vitest";
import { executeDeckMutation, type DeckMutation } from "./open-deck-mutations";
const id = "019f68cb-9400-7000-8000-000000000001";
const schedule: DeckMutation = {
  mutationId: id,
  operation: "create-schedule",
  title: "Planning",
  notes: "note",
  startsAt: "2026-10-22T01:00:00Z",
  endsAt: "2026-10-22T02:00:00Z",
};
const task: DeckMutation = {
  mutationId: id,
  operation: "complete-task",
  itemId: id,
  expectedVersion: 7,
  completionNote: "Done",
};
const response = (data: unknown, status = 200) =>
  new Response(JSON.stringify(data), { status });
describe("tablet writes", () => {
  it("does not bypass pending owner confirmation or retry a rejected completion", async () => {
    let calls = 0;
    const result = await executeDeckMutation(
      "https://example.test",
      "fixture",
      task,
      async () => {
        calls++;
        return calls === 1
          ? response({ id, status: "open", version: 7 })
          : response(
              { error: { code: "task.completion_review_pending" } },
              409,
            );
      },
    );
    expect(result).toEqual({ error: "CONFLICT" });
    expect(calls).toBe(2);
  });
  it("creates schedules with a stable server mutation id and Seoul timezone", async () => {
    const calls: RequestInit[] = [];
    const result = await executeDeckMutation(
      "https://example.test",
      "private",
      schedule,
      async (_url, init) => {
        calls.push(init!);
        return response(calls.length === 1 ? { items: [] } : { id });
      },
    );
    expect(result).toEqual({ entityId: id });
    expect(JSON.parse(calls[1].body as string)).toMatchObject({
      clientMutationId: id,
      title: "Planning",
      timeZone: "Asia/Seoul",
      projectId: null,
      taskId: null,
    });
  });
  it("reconciles an accepted schedule after a lost response without creating it again", async () => {
    let posts = 0;
    const result = await executeDeckMutation(
      "https://example.test",
      "private",
      schedule,
      async (_url, init) => {
        if (init?.method === "POST") posts++;
        return response({ items: [{ ...schedule, id }] });
      },
    );
    expect(result).toEqual({ entityId: id });
    expect(posts).toBe(0);
  });
  it("does not write stale tasks or resend a completion note for an already completed task", async () => {
    for (const state of [
      { id, status: "open", version: 8 },
      { id, status: "completed", version: 8 },
    ]) {
      let count = 0;
      const result = await executeDeckMutation(
        "https://example.test",
        "private",
        task,
        async () => {
          count++;
          return response(state);
        },
      );
      expect(count).toBe(1);
      expect(result).toEqual(
        state.status === "completed"
          ? { entityId: id, alreadyCompleted: true }
          : { error: "CONFLICT" },
      );
    }
  });
  it("passes the version fence and optional completion note to the existing API", async () => {
    const calls: RequestInit[] = [];
    const result = await executeDeckMutation(
      "https://example.test",
      "private",
      task,
      async (_url, init) => {
        calls.push(init!);
        return response(
          calls.length === 1
            ? { id, status: "open", version: 7 }
            : { id, status: "completed", version: 8 },
        );
      },
    );
    expect(result).toEqual({ entityId: id });
    expect(JSON.parse(calls[1].body as string)).toEqual({
      expectedVersion: 7,
      completionNote: "Done",
    });
  });
  it("preserves an ambiguous write result and delegates expired authentication to the coordinator", async () => {
    let count = 0;
    const result = await executeDeckMutation(
      "https://example.test",
      "private",
      schedule,
      async () => {
        if (++count === 1) return response({ items: [] });
        throw Error("response lost");
      },
    );
    expect(result).toEqual({ error: "UNKNOWN_RESULT" });
    await expect(
      executeDeckMutation("https://example.test", "private", task, async () =>
        response({}, 401),
      ),
    ).rejects.toMatchObject({ code: "unauthorized" });
  });
});
