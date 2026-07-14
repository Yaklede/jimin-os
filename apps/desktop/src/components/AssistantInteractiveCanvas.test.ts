import { describe, expect, it } from "vitest";

import { type AssistantPresentationItem } from "../api/agent";
import { canOpenPresentationItem } from "./AssistantInteractiveCanvas";

function project(status: "active" | "removed"): AssistantPresentationItem {
  return {
    type: "project",
    id: "019bffff-ffff-7fff-8000-000000000001",
    workspaceId: "019bffff-ffff-7fff-8000-000000000002",
    title: "비스킷링크",
    status,
    objective: null,
    nextAction: null,
    riskLevel: 0,
    openTaskCount: 0,
  };
}

describe("assistant project result navigation", () => {
  it("does not offer navigation after the project has been removed", () => {
    expect(canOpenPresentationItem(project("active"))).toBe(true);
    expect(canOpenPresentationItem(project("removed"))).toBe(false);
  });
});
