import { renderToStaticMarkup } from "react-dom/server";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { AppearanceControl } from "./appearance-control";
import { RepresentativeImage } from "./RepresentativeImage";

it("retains the original image when no saved preference is available", () => {
  expect(renderToStaticMarkup(<RepresentativeImage />)).toContain(
    'src="/images/hamster-home-wave.png"',
  );
});
it("permits local image decoding in native production without allowing plugins", () => {
  const config = JSON.parse(
    readFileSync(
      new URL("../../src-tauri/tauri.conf.json", import.meta.url),
      "utf8",
    ),
  );
  expect(config.app.security.csp).toContain(
    "img-src 'self' data: blob: https:",
  );
  expect(config.app.security.csp).toContain("object-src 'none'");
});
it("adds image controls without dropping modes, colors or the existing popover", () => {
  const markup = renderToStaticMarkup(<AppearanceControl />);
  for (const label of [
    "화면 꾸미기",
    "라이트",
    "다크",
    "노랑",
    "민트",
    "파랑",
    "보라",
    "분홍",
    "대표 이미지",
    "이미지 바꾸기",
    "기본 이미지로 되돌리기",
    "이 기기에만 저장돼요.",
  ])
    expect(markup).toContain(label);
  expect(markup).toContain('accept="image/png,image/jpeg,image/webp"');
  expect(markup).toContain('popover="auto"');
});
it("sizes the appearance popover to its content and prevents stretched grid rows", () => {
  const css = readFileSync(new URL("../styles.css", import.meta.url), "utf8");
  expect(css).toMatch(/\.appearance-panel\s*\{[^}]*height: fit-content;/);
  expect(css).toMatch(
    /\.appearance-panel:popover-open\s*\{[^}]*grid-auto-rows: max-content;[^}]*align-content: start;/,
  );
  expect(css).toMatch(/\.appearance-panel__mode\s*\{[^}]*height: 44px;/);
  expect(css).toMatch(/\.appearance-panel__color\s*\{[^}]*height: 68px;/);
});
