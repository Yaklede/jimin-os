import { readFileSync } from "node:fs";
import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { OsShell } from "./OsShell";

describe("OS shell platform layout", () => {
  it("marks a resizable macOS shell as desktop", () => {
    expect(renderShell("desktop")).toContain('data-platform="desktop"');
  });

  it("marks native phone shells independently from viewport width", () => {
    expect(renderShell("android")).toContain('data-platform="android"');
    expect(renderShell("ios")).toContain('data-platform="ios"');
  });

  it("keeps the wide desktop grid and a compact desktop rail at narrow widths", () => {
    const styles = readFileSync(
      new URL("../styles.css", import.meta.url),
      "utf8",
    );

    expect(styles).toMatch(
      /\.os-shell\s*\{[\s\S]*?grid-template-columns:\s*216px minmax\(0, 1fr\)/,
    );
    expect(styles).toMatch(
      /\.os-shell:is\(\[data-platform="desktop"\], \[data-platform="web"\]\)\s*\{[\s\S]*?grid-template-columns:\s*72px minmax\(0, 1fr\)/,
    );
    expect(styles).toMatch(
      /\.os-shell:is\(\[data-platform="desktop"\], \[data-platform="web"\]\)[\s\S]*?\.os-mobile-nav\s*\{[\s\S]*?display:\s*none/,
    );
  });
});

function renderShell(
  platform: NonNullable<ComponentProps<typeof OsShell>["platform"]>,
): string {
  const props: ComponentProps<typeof OsShell> = {
    destination: "home",
    platform,
    onNavigate: () => undefined,
    onVoiceTranscript: () => undefined,
    onVoiceCommand: async () => ({
      kind: "conversation",
      message: "대화에서 이어갈게요.",
    }),
    onRefresh: () => undefined,
    refreshing: false,
    children: createElement("div", null, "content"),
  };

  return renderToStaticMarkup(createElement(OsShell, props));
}
