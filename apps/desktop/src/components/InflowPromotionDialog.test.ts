import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  InflowPromotionDialog,
  inflowPromotionCloseAction,
  inflowPromotionDialogCopy,
} from "./InflowPromotionDialog";

describe("inflow registration dialog", () => {
  it("does not mount a form before the user opens registration", () => {
    expect(
      renderToStaticMarkup(
        createElement(InflowPromotionDialog, {
          open: false,
          busy: false,
          dirty: false,
          onClose() {},
          children: "form",
        }),
      ),
    ).toBe("");
  });
  it("labels the modal and renders registration as a separate surface", () => {
    const markup = renderToStaticMarkup(
      createElement(InflowPromotionDialog, {
        open: true,
        busy: false,
        dirty: false,
        onClose() {},
        children: "registration fields",
      }),
    );
    expect(markup).toContain("<dialog");
    expect(markup).toContain(inflowPromotionDialogCopy.title);
    expect(markup).toContain(inflowPromotionDialogCopy.description);
    expect(markup).toContain("registration fields");
  });
  it("requires confirmation for modified input and blocks closing during submission", () => {
    expect(inflowPromotionCloseAction(false, false)).toBe("close");
    expect(inflowPromotionCloseAction(false, true)).toBe("confirm");
    expect(inflowPromotionCloseAction(true, true)).toBe("blocked");
    expect(inflowPromotionCloseAction(true, false)).toBe("blocked");
  });
  it("disables the close button and announces a pending submission", () => {
    const markup = renderToStaticMarkup(
      createElement(InflowPromotionDialog, {
        open: true,
        busy: true,
        dirty: true,
        onClose() {},
        children: "form",
      }),
    );
    expect(markup).toContain('aria-busy="true"');
    expect(markup).toContain('disabled=""');
  });
});
