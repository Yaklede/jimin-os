import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  earlierSyncCursor,
  laterSyncCursor,
  readSyncCursor,
  writeSyncCursor,
} from "./sync-cursor";

const values = new Map<string, string>();

beforeEach(() => {
  values.clear();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => values.set(key, value),
    clear: () => values.clear(),
  });
});

describe("durable sync cursor", () => {
  it("keeps the later server sequence without number precision loss", () => {
    expect(laterSyncCursor("9007199254740993", "9007199254740992")).toBe(
      "9007199254740993",
    );
  });

  it("uses an earlier durable cursor after a restored server", () => {
    expect(earlierSyncCursor("105", "91")).toBe("91");
    expect(earlierSyncCursor(undefined, "91")).toBe("91");
  });

  it("persists only valid non-negative sequences", () => {
    writeSyncCursor("72");
    writeSyncCursor("-1");
    expect(readSyncCursor()).toBe("72");
  });
});
