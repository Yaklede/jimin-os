import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

it("separates the home grid from its sibling history with the section spacing token", () => {
  const styles = readFileSync(
    new URL("../styles.css", import.meta.url),
    "utf8",
  );
  expect(styles).toMatch(
    /\.home-page\s*\+\s*\.home-review-actions\s*\{[^}]*margin-block-start:\s*var\(--space-section\)/,
  );
  expect(styles).toMatch(
    /\.home-page\s*\+\s*\.home-review-actions\s*\{[^}]*width:\s*min\(100%, 1280px\);[^}]*margin-inline:\s*auto/,
  );
});

it("gives recovery cards a shared surface and spacing inside the collapsed history", () => {
  const styles = readFileSync(
    new URL("./inflowRecovery.css", import.meta.url),
    "utf8",
  );
  expect(styles).toMatch(
    /\.inflow-recovery\s*\{[^}]*border:\s*1px solid var\(--border\)/,
  );
  expect(styles).toMatch(
    /\.inflow-recovery\s*\{[^}]*border-radius:\s*var\(--radius-panel\)/,
  );
  const homeStyles = readFileSync(
    new URL("../styles.css", import.meta.url),
    "utf8",
  );
  expect(homeStyles).toMatch(
    /\.home-review-actions__recoveries\s*\{[^}]*gap:\s*var\(--space-section\)/,
  );
});
