import { readFileSync } from "node:fs";
import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { afterEach, describe, expect, it, vi } from "vitest";

import { OsShell } from "./OsShell";

describe("OS shell platform layout", () => {
  afterEach(() => vi.unstubAllGlobals());

  it.each([
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 CriOS/140.0.0.0 Mobile/15E148 Safari/604.1",
    "Mozilla/5.0 (Linux; Android 16; Pixel 9) AppleWebKit/537.36 Chrome/140.0.0.0 Mobile Safari/537.36",
  ])(
    "automatically renders the web navigation for a mobile browser: %s",
    (userAgent) => {
      vi.stubGlobal("navigator", { userAgent });
      const markup = renderShell();

      expect(markup).toContain('data-platform="web"');
      expect(markup).toContain('class="os-mobile-more"');
      expect(markup).toContain("더보기");
    },
  );

  it("marks a resizable macOS shell as desktop", () => {
    expect(renderShell("desktop")).toContain('data-platform="desktop"');
  });

  it("marks native phone shells independently from viewport width", () => {
    expect(renderShell("android")).toContain('data-platform="android"');
    expect(renderShell("ios")).toContain('data-platform="ios"');
  });

  it.each(["web", "android", "ios"] as const)(
    "keeps four destinations around the central voice action and More in the header on %s",
    (platform) => {
      const markup = renderShell(platform);
      const bottom = markup.slice(
        markup.indexOf('<nav class="os-mobile-nav"'),
        markup.lastIndexOf("</nav>") + 6,
      );
      const header = markup.slice(
        markup.indexOf("<header"),
        markup.indexOf('class="os-page-load"'),
      );
      expect(bottom).toContain("회의");
      expect(bottom).not.toContain("더보기");
      expect(bottom.match(/class="os-nav__button /g)).toHaveLength(4);
      expect(header).toContain('class="os-mobile-more"');
      expect(header).toContain('aria-label="더보기"');
      expect(header).toContain("기억");
      expect(header).toContain("설정");
      expect(bottom).toContain("os-mobile-nav__assistant");
      expect(bottom).toContain('aria-label="지민에게 말하기"');
      expect(bottom.indexOf("프로젝트")).toBeLessThan(bottom.indexOf("일정"));
      expect(bottom.indexOf("프로젝트")).toBeLessThan(
        bottom.indexOf('aria-label="지민에게 말하기"'),
      );
      expect(bottom.indexOf('aria-label="지민에게 말하기"')).toBeLessThan(
        bottom.indexOf("일정"),
      );
      expect(bottom.indexOf("일정")).toBeLessThan(bottom.indexOf("회의"));
    },
  );

  it("keeps the wide desktop grid and a compact desktop rail at narrow widths", () => {
    const styles = readFileSync(
      new URL("../styles.css", import.meta.url),
      "utf8",
    );

    expect(styles).toMatch(
      /\.os-shell\s*\{[\s\S]*?grid-template-columns:\s*216px minmax\(0, 1fr\)/,
    );
    expect(styles).toMatch(
      /\.os-shell\[data-platform="desktop"\]\s*\{[\s\S]*?grid-template-columns:\s*72px minmax\(0, 1fr\)/,
    );
    expect(styles).toMatch(
      /\.os-shell\[data-platform="desktop"\][\s\S]*?\.os-mobile-nav\s*\{[\s\S]*?display:\s*none/,
    );
  });
});

function renderShell(
  platform?: NonNullable<ComponentProps<typeof OsShell>["platform"]>,
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
