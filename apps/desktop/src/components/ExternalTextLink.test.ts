import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";

import {
  handleExternalLinkClick,
  LinkifiedText,
  stripTrailingUrlPunctuation,
  trustedExternalUrl,
} from "./ExternalTextLink";

describe("external text links", () => {
  it("opens an HTTPS link through the Tauri opener when it is clicked", async () => {
    const preventDefault = vi.fn();
    const openTauri = vi.fn(async () => undefined);

    handleExternalLinkClick(
      { preventDefault },
      "https://itsm.example.com/issues/123",
      {
        tauri: true,
        openTauri,
        openWeb: vi.fn(),
      },
    );
    await Promise.resolve();

    expect(preventDefault).toHaveBeenCalledOnce();
    expect(openTauri).toHaveBeenCalledWith(
      "https://itsm.example.com/issues/123",
    );
  });

  it("linkifies plain HTTPS URLs while keeping unsafe schemes as text", () => {
    const markup = renderToStaticMarkup(
      createElement(LinkifiedText, {
        text: "원문 https://docs.example.com/spec. file:///etc/passwd",
      }),
    );

    expect(markup).toContain('href="https://docs.example.com/spec"');
    expect(markup).toContain("file:///etc/passwd");
    expect(markup).not.toContain('href="file:///etc/passwd"');
  });

  it("rejects non-HTTPS schemes", () => {
    expect(trustedExternalUrl("javascript:alert(1)")).toBeUndefined();
    expect(trustedExternalUrl("http://example.com")).toBeUndefined();
  });

  it("keeps balanced URL brackets and removes sentence punctuation", () => {
    expect(
      stripTrailingUrlPunctuation("https://example.com/docs/(draft)"),
    ).toBe("https://example.com/docs/(draft)");
    expect(
      stripTrailingUrlPunctuation("https://example.com/issues/123)."),
    ).toBe("https://example.com/issues/123");
  });
});
