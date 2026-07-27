import { describe, expect, it } from "vitest";

import { LatestRequestGate } from "./latestRequestGate";

describe("LatestRequestGate", () => {
  it("rejects a response that finishes after a newer request starts", () => {
    const gate = new LatestRequestGate();
    const first = gate.begin();
    const second = gate.begin();

    expect(gate.isCurrent(first)).toBe(false);
    expect(gate.isCurrent(second)).toBe(true);
  });

  it("rejects every pending response after a local state transition", () => {
    const gate = new LatestRequestGate();
    const pending = gate.begin();

    gate.invalidate();

    expect(gate.isCurrent(pending)).toBe(false);
  });
});
