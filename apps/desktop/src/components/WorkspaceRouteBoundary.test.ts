import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  WorkspaceRouteBoundary,
  WorkspaceRouteErrorFallback,
} from "./WorkspaceRouteBoundary";

describe("workspace route recovery", () => {
  it("keeps route content visible while there is no render failure", () => {
    const markup = renderToStaticMarkup(
      createElement(WorkspaceRouteBoundary, {
        onRetry: () => undefined,
        children: createElement("p", null, "일정"),
        loadingFallback: createElement("p", null, "불러오는 중"),
      }),
    );

    expect(markup).toContain("일정");
    expect(markup).not.toContain('role="alert"');
  });

  it("provides an accessible reload action after a route failure", () => {
    const markup = renderToStaticMarkup(
      createElement(WorkspaceRouteErrorFallback, {
        onRetry: () => undefined,
      }),
    );

    expect(markup).toContain('role="alert"');
    expect(markup).toContain("화면을 불러오지 못했어요");
    expect(markup).toContain("다시 불러오기");
    expect(markup).toContain("<button");
  });

  it("turns render errors into the recovery state", () => {
    expect(WorkspaceRouteBoundary.getDerivedStateFromError()).toEqual({
      failed: true,
    });
  });
});
