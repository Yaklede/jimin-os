import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const styles = readFileSync(
  new URL("./mobileWorkspace.css", import.meta.url),
  "utf8",
);

describe("native phone layout", () => {
  it("shares safe-area layout across web, Android and iOS, not native desktop", () => {
    expect(styles).toContain('.os-shell:not([data-platform="desktop"])');
    expect(styles).toContain(
      "--mobile-top-clearance: max(12px, var(--safe-area-top))",
    );
    expect(styles).toContain(
      "--mobile-nav-height: calc(56px + max(8px, var(--safe-area-bottom)))",
    );
    expect(styles).toContain(
      "padding-bottom: calc(var(--mobile-nav-height) + 16px)",
    );
    expect(styles).toContain(
      "scroll-padding-bottom: calc(var(--mobile-nav-height) + 16px)",
    );
  });

  it("keeps the fixed navigation opaque and reserves its full height", () => {
    const navigation = styles.match(
      /\.os-shell:not\(\[data-platform="desktop"\]\) \.os-mobile-nav \{([^}]+)\}/,
    )?.[1];
    expect(navigation).toContain("height: var(--mobile-nav-height)");
    expect(navigation).toContain("background: var(--surface)");
    expect(navigation).not.toContain("transparent");
  });

  it("keeps the More menu away from the physical screen edge", () => {
    const menu = styles.match(/\.os-mobile-more__items \{([^}]+)\}/)?.[1];
    expect(menu).toContain("right: 16px");
    expect(menu).toContain("calc(100vw - 32px)");
  });

  it("uses five equal destinations with no unlabeled floating action", () => {
    expect(styles).toContain(
      "grid-template-columns: repeat(5, minmax(0, 1fr))",
    );
    const button = styles.match(
      /\.os-shell:not\(\[data-platform="desktop"\]\) \.os-mobile-nav \.os-nav__button \{([^}]+)\}/,
    )?.[1];
    expect(button).toContain("flex-direction: column");
    expect(button).toContain("justify-content: center");
    expect(button).toContain("font-size: 12px");
    expect(button).toContain("font-weight: 500");
    expect(button).toContain("height: 56px");
    expect(button).toContain("background: transparent");
    expect(styles).not.toContain("os-mobile-nav__assistant");
  });

  it("prevents desktop greeting motion and line heights leaking onto phones", () => {
    const greeting = styles.match(
      /\.os-shell:not\(\[data-platform="desktop"\]\) \.home-greeting \{([^}]+)\}/,
    )?.[1];
    expect(greeting).toContain("transform: none");
    expect(greeting).toContain("min-height: 0");
    expect(styles).toMatch(/\.home-greeting h1 \{[^}]*line-height: 28px/);
    expect(styles).toContain("clip-path: inset(50%)");
  });
});
