import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { designPreviewInflow } from "../design-preview-inflow";
import { inflowRecoveryCopy as text } from "../copy/inflowRecovery";
import { InflowRecoveryPanel } from "./InflowRecoveryPanel";

describe("source delivery recovery", () => {
  function render(overrides = {}) {
    return renderToStaticMarkup(
      createElement(InflowRecoveryPanel, {
        items: [
          {
            ...designPreviewInflow()[0],
            status: "promoted",
            promotedTaskId: "task",
            completionStatus: "failed",
            completionReactionCompleted: true,
          },
        ],
        saving: false,
        onRetry: async () => {},
        onOpenTask: async () => {},
        ...overrides,
      } as Parameters<typeof InflowRecoveryPanel>[0]),
    );
  }
  it("starts compact and shows saved, successful and failed stages separately", () => {
    const markup = render();
    expect(markup).toContain('<details class="inflow-recovery">');
    expect(markup).toContain(text.savedTask);
    expect(markup).toContain(text.reaction);
    expect(markup).toContain(text.reply);
    expect(markup.match(/data-state="done"/g)).toHaveLength(2);
    expect(markup.match(/data-state="failed"/g)).toHaveLength(1);
    expect(markup).toContain(text.retry);
  });
  it("blocks conflicting retries while saving", () => {
    expect(
      render({ saving: true })
        .match(/<button[^>]+>/g)
        ?.every((button) => button.includes('disabled=""')),
    ).toBe(true);
  });
  it("has no panel when all source work is done", () => {
    expect(render({ items: [] })).toBe("");
  });
});
