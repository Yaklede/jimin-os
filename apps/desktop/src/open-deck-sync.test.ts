import { describe, it, expect, vi } from "vitest";
import { DeckSnapshotPublisher, safeDeckDiagnostic } from "./open-deck-sync";
import { normalizeDeckSnapshot, loadDeckSnapshot } from "./open-deck-bridge";

describe("companion refresh recovery", () => {
  it("keeps the original collection after timeouts, backs off and restores fresh writable data", async () => {
    let now = 1000000;
    const first = normalizeDeckSnapshot({ tasks: [] }, [], [], new Date(now));
    const failure = Object.assign(new Error("PRIVATE_TOKEN"), {
      diagnostic: {
        route: "/v1/home",
        code: "timeout",
        httpStatus: 0,
        durationMs: 6000,
      },
    });
    const recoveredAt = now + 20000;
    const load = vi
      .fn()
      .mockResolvedValueOnce(first)
      .mockRejectedValueOnce(failure)
      .mockRejectedValueOnce(failure)
      .mockResolvedValueOnce({ ...first, updatedAt: recoveredAt });
    const publish = vi.fn().mockResolvedValue(undefined),
      log = vi.fn().mockResolvedValue(undefined);
    const worker = new DeckSnapshotPublisher(load, publish, log, () => now);
    expect(await worker.refresh()).toBe(15000);
    now += 15000;
    expect(await worker.refresh()).toBe(2000);
    let cached = publish.mock.calls.at(-1)![0];
    expect(cached.updatedAt).toBe(first.updatedAt);
    expect(cached.connected).toBe(false);
    expect(cached.capabilities.taskComplete).toBe(false);
    expect(cached.sync.retryAt).toBe(now + 2000);
    now += 2000;
    expect(await worker.refresh()).toBe(5000);
    expect(publish.mock.calls.at(-1)![0].updatedAt).toBe(first.updatedAt);
    expect(await worker.refresh()).toBe(15000);
    cached = publish.mock.calls.at(-1)![0];
    expect(cached.connected).toBe(true);
    expect(cached.sync.state).toBe("fresh");
    expect(cached.capabilities.taskComplete).toBe(true);
    expect(log.mock.calls.at(-1)![0].event).toBe("recovered");
    expect(JSON.stringify(log.mock.calls)).not.toContain("PRIVATE_");
  });
  it("coalesces overlapping triggers and prevents publishing after disposal", async () => {
    let release!: (value: ReturnType<typeof normalizeDeckSnapshot>) => void;
    const load = vi.fn(
      () =>
        new Promise<ReturnType<typeof normalizeDeckSnapshot>>((resolve) => {
          release = resolve;
        }),
    );
    const publish = vi.fn(),
      log = vi.fn();
    const worker = new DeckSnapshotPublisher(load, publish, log);
    const a = worker.refresh(),
      b = worker.refresh();
    expect(a).toBe(b);
    expect(load).toHaveBeenCalledTimes(1);
    worker.dispose();
    release(normalizeDeckSnapshot({}, [], []));
    await a;
    expect(publish).not.toHaveBeenCalled();
  });
  it("does not invent a collection timestamp before the first successful read; logging failure cannot block retry", async () => {
    const publish = vi.fn().mockResolvedValue(undefined);
    const worker = new DeckSnapshotPublisher(
      async () => {
        throw Error("PRIVATE_SECRET");
      },
      publish,
      async () => {
        throw Error("disk full");
      },
    );
    expect(await worker.refresh()).toBe(2000);
    expect(publish.mock.calls[0][0].updatedAt).toBe(0);
    expect(JSON.stringify(publish.mock.calls)).not.toContain("PRIVATE");
  });
  it("waits for sibling reads before rejecting and reports only safe API diagnostics", async () => {
    let release!: (response: Response) => void;
    const logs: string[] = [];
    const request = loadDeckSnapshot(
      "https://os.example",
      "PRIVATE_TOKEN",
      new Date(),
      async (input) => {
        const route = String(input);
        if (route.includes("schedule-entries"))
          return new Promise<Response>((r) => {
            release = r;
          });
        if (route.includes("scheduled-work"))
          return new Response('{"items":[]}');
        return new Response("{}", { status: 503 });
      },
    );
    request.catch(() => logs.push("failed"));
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(logs).toEqual([]);
    release(new Response('{"items":[]}'));
    const error = await request.catch((cause) => cause);
    expect(safeDeckDiagnostic(error)).toMatchObject({
      route: "/v1/home",
      code: "http",
      httpStatus: 503,
    });
    expect(JSON.stringify(safeDeckDiagnostic(error))).not.toContain(
      "PRIVATE_TOKEN",
    );
    expect(
      safeDeckDiagnostic({
        diagnostic: { route: "https://private?token=secret", code: "PRIVATE" },
      }).route,
    ).toBe("snapshot");
  });
});
