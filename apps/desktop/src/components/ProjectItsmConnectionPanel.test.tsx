import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { copy } from "../copy";
import { ProjectItsmConnectionPanel } from "./ProjectItsmConnectionPanel";

describe("project ITSM connection panel", () => {
  it("offers a project connection without exposing configuration details", () => {
    const markup = renderPanel({
      snapshot: { available: true, item: null },
    });

    expect(markup).toContain(copy.projects.itsmConnect);
    expect(markup).toContain(copy.projects.itsmAvailableDescription);
    expect(markup).not.toContain("ITSM 프로젝트 번호");
    expect(markup).not.toContain("<input");
    expect(markup).not.toContain(["to", "ken"].join(""));
    expect(markup).not.toContain(["base", "Url"].join(""));
    expect(markup).not.toContain("https://");
  });

  it("shows a compact connected state and safe actions", () => {
    const markup = renderPanel({
      snapshot: {
        available: true,
        item: {
          id: "connection-1",
          projectId: "project-1",
          enabled: true,
          confirmationStatus: "confirmed",
          candidateProjectName: null,
          version: 2,
        },
      },
    });

    expect(markup).toContain(copy.projects.itsmConnected);
    expect(markup).toContain(copy.projects.itsmReload);
    expect(markup).toContain(copy.projects.itsmDisconnect);
    expect(markup).not.toContain("connection-1");
    expect(markup).not.toContain("project-1");
  });

  it("asks the owner to confirm the detected project by name", () => {
    const markup = renderPanel({
      snapshot: {
        available: true,
        item: {
          id: "connection-1",
          projectId: "project-1",
          enabled: true,
          confirmationStatus: "confirmation_required",
          candidateProjectName: "비스킷링크",
          version: 2,
        },
      },
    });

    expect(markup).toContain(copy.projects.itsmCandidateTitle("비스킷링크"));
    expect(markup).toContain(copy.projects.itsmConfirm);
    expect(markup).toContain(copy.projects.itsmDisconnect);
    expect(markup).not.toContain("project-1");
  });

  it("explains the automatic discovery state after opt-in", () => {
    const markup = renderPanel({
      snapshot: {
        available: true,
        item: {
          id: "connection-1",
          projectId: "project-1",
          enabled: true,
          confirmationStatus: "discovering",
          candidateProjectName: null,
          version: 1,
        },
      },
    });

    expect(markup).toContain(copy.projects.itsmDiscoveringHelp);
    expect(markup).toContain(copy.projects.itsmDisconnect);
  });

  it("explains that server preparation is needed without internal terms", () => {
    const markup = renderPanel({
      snapshot: { available: false, item: null },
    });

    expect(markup).toContain(copy.projects.itsmNeedsSetupTitle);
    expect(markup).toContain(copy.projects.itsmNeedsSetupDescription);
    expect(markup).toContain(copy.projects.itsmReload);
    expect(markup).not.toContain(["end", "point"].join(""));
    expect(markup).not.toContain(["creden", "tial"].join(""));
  });

  it("pairs a recoverable load message with the next action", () => {
    const markup = renderPanel({
      snapshot: undefined,
      problemMessage: copy.projects.itsmLoadProblem,
    });

    expect(markup).toContain(copy.projects.itsmLoadProblem);
    expect(markup).toContain(copy.projects.itsmReload);
  });
});

function renderPanel(
  overrides: Partial<ComponentProps<typeof ProjectItsmConnectionPanel>> = {},
): string {
  const props: ComponentProps<typeof ProjectItsmConnectionPanel> = {
    snapshot: undefined,
    loading: false,
    saving: false,
    problemMessage: undefined,
    onReload: async () => undefined,
    onConnect: async () => undefined,
    onConfirm: async () => undefined,
    onDisconnect: async () => undefined,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(ProjectItsmConnectionPanel, props));
}
