import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { scheduledWorkCopy } from "../copy/scheduledWork";
import { ScheduledWorkCloseConfirmation } from "./ScheduledWorkCloseConfirmation";

describe("scheduled work close confirmation", () => {
  const props = { onKeepEditing() {}, onDiscard() {} };

  it("does not show a confirmation when closing unmodified settings", () => {
    expect(
      renderToStaticMarkup(
        createElement(ScheduledWorkCloseConfirmation, {
          ...props,
          open: false,
        }),
      ),
    ).toBe("");
  });

  it("puts the discard question and explicit choices in a separate accessible dialog", () => {
    const markup = renderToStaticMarkup(
      createElement(ScheduledWorkCloseConfirmation, { ...props, open: true }),
    );
    expect(markup).toContain('role="alertdialog"');
    expect(markup).toContain(`aria-label="${scheduledWorkCopy.discardTitle}"`);
    expect(markup).toContain("aria-describedby=");
    expect(markup).toContain(scheduledWorkCopy.closeHelp);
    expect(markup).toContain(scheduledWorkCopy.keepEditing);
    expect(markup).toContain(scheduledWorkCopy.discard);
    expect(markup).not.toContain("scheduled-form");
  });
});
