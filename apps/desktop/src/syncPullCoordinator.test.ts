import { describe, expect, it, vi } from "vitest";

import { SyncPullCoordinator } from "./syncPullCoordinator";

describe("sync pull coordinator", () => {
  it("preserves one trailing pull when a cursor arrives during refresh", async () => {
    const coordinator = new SyncPullCoordinator();
    let releaseFirst: (() => void) | undefined;
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const firstOperation = vi.fn(async () => {
      await firstBlocked;
    });
    const trailingOperation = vi.fn(async () => undefined);

    const first = coordinator.request(firstOperation);
    const trailing = coordinator.request(trailingOperation);
    releaseFirst?.();
    await Promise.all([first, trailing]);

    expect(firstOperation).toHaveBeenCalledTimes(1);
    expect(trailingOperation).toHaveBeenCalledTimes(1);
  });

  it("coalesces repeated overlapping pulses into one trailing refresh", async () => {
    const coordinator = new SyncPullCoordinator();
    let releaseFirst: (() => void) | undefined;
    const firstBlocked = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });
    const firstOperation = vi.fn(async () => {
      await firstBlocked;
    });
    const trailingOperation = vi.fn(async () => undefined);

    const pull = coordinator.request(firstOperation);
    coordinator.request(trailingOperation);
    coordinator.request(trailingOperation);
    coordinator.request(trailingOperation);
    releaseFirst?.();
    await pull;

    expect(firstOperation).toHaveBeenCalledTimes(1);
    expect(trailingOperation).toHaveBeenCalledTimes(1);
  });

  it("clears a failed pull so a later reconciliation can recover", async () => {
    const coordinator = new SyncPullCoordinator();

    await expect(
      coordinator.request(async () => {
        throw new Error("offline");
      }),
    ).rejects.toThrow("offline");

    const recovered = vi.fn(async () => undefined);
    await coordinator.request(recovered);
    expect(recovered).toHaveBeenCalledTimes(1);
  });
});
