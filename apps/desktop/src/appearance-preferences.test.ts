import { afterEach, describe, expect, it, vi } from "vitest";
import {
  appearanceColors,
  appearancePalettes,
  appearanceStorageKey,
  applyAppearance,
  loadAppearancePreferences,
  saveAppearancePreferences,
} from "./appearance-preferences";

afterEach(() => vi.unstubAllGlobals());

it("synchronizes only the selected visual mode with the Android system bars", () => {
  const setMode = vi.fn();
  const root = { dataset: {}, style: { setProperty: vi.fn() } };
  vi.stubGlobal("document", {
    documentElement: root,
    querySelectorAll: () => [],
  });
  vi.stubGlobal("window", { JiminAppearance: { setMode } });
  applyAppearance({ mode: "dark", color: "yellow" });
  applyAppearance({ mode: "light", color: "blue" });
  expect(setMode.mock.calls).toEqual([["dark"], ["light"]]);
});

function memoryStorage(initial?: string) {
  const values = new Map<string, string>(
    initial ? [[appearanceStorageKey, initial]] : [],
  );
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => {
      values.set(key, value);
    },
  };
}

describe("appearance preferences", () => {
  it("uses blue for a fresh light appearance and keeps the dark yellow default", () => {
    expect(loadAppearancePreferences(memoryStorage(), "light")).toEqual({
      mode: "light",
      color: "blue",
    });
    expect(loadAppearancePreferences(undefined, "dark")).toEqual({
      mode: "dark",
      color: "yellow",
    });
  });

  it("restores both choices across reload with a different launch default", () => {
    const storage = memoryStorage();
    expect(
      saveAppearancePreferences(storage, { mode: "light", color: "blue" }),
    ).toBe(true);
    expect(loadAppearancePreferences(storage, "dark")).toEqual({
      mode: "light",
      color: "blue",
    });
    saveAppearancePreferences(storage, { mode: "dark", color: "pink" });
    expect(loadAppearancePreferences(storage, "light")).toEqual({
      mode: "dark",
      color: "pink",
    });
  });

  it("recovers from malformed or unavailable browser storage", () => {
    expect(loadAppearancePreferences(memoryStorage("{"), "light")).toEqual({
      mode: "light",
      color: "blue",
    });
    const blocked = {
      getItem() {
        throw new Error("blocked");
      },
      setItem() {
        throw new Error("blocked");
      },
    };
    expect(loadAppearancePreferences(blocked, "dark")).toEqual({
      mode: "dark",
      color: "yellow",
    });
    expect(
      saveAppearancePreferences(blocked, { mode: "light", color: "mint" }),
    ).toBe(false);
    expect(
      saveAppearancePreferences(undefined, { mode: "light", color: "mint" }),
    ).toBe(false);
  });

  it("validates each saved choice without discarding a valid companion choice", () => {
    expect(
      loadAppearancePreferences(
        memoryStorage('{"mode":"system","color":"purple"}'),
        "light",
      ),
    ).toEqual({ mode: "light", color: "purple" });
    expect(
      loadAppearancePreferences(
        memoryStorage('{"mode":"light","color":"invalid"}'),
        "dark",
      ),
    ).toEqual({ mode: "light", color: "yellow" });
    for (const value of ["null", "[]", '"dark"', "42"]) {
      expect(loadAppearancePreferences(memoryStorage(value), "dark")).toEqual({
        mode: "dark",
        color: "yellow",
      });
    }
  });

  for (const color of appearanceColors) {
    for (const mode of ["light", "dark"] as const) {
      it(`${mode}/${color} follows its approved color and contrast contract`, () => {
        const palette = appearancePalettes[color][mode];
        // The user explicitly chose this brighter blue with white action text.
        // Keep this exception visible; it does not meet normal-text AA contrast.
        if (mode === "light" && color === "blue") {
          expect(palette.accent).toBe("#4594fb");
          expect(palette.label).toBe(palette.accent);
          expect(palette.onAccent).toBe("#ffffff");
          expect(
            contrast(palette.accent, palette.onAccent),
          ).toBeGreaterThanOrEqual(3);
          expect(contrast(palette.accent, palette.onAccent)).toBeLessThan(4.5);
          expect(
            contrast(palette.hover, palette.onAccent),
          ).toBeGreaterThanOrEqual(3);
          return;
        }
        const surface = mode === "light" ? "#ffffff" : "#333333";
        expect(
          contrast(palette.accent, palette.onAccent),
        ).toBeGreaterThanOrEqual(4.5);
        expect(
          contrast(palette.hover, palette.onAccent),
        ).toBeGreaterThanOrEqual(4.5);
        expect(contrast(palette.label, surface)).toBeGreaterThanOrEqual(4.5);
        const selectedSurface =
          "#" +
          [1, 3, 5]
            .map((position) =>
              Math.round(
                parseInt(palette.accent.slice(position, position + 2), 16) *
                  0.2 +
                  parseInt(surface.slice(position, position + 2), 16) * 0.8,
              )
                .toString(16)
                .padStart(2, "0"),
            )
            .join("");
        expect(contrast(palette.label, selectedSurface)).toBeGreaterThanOrEqual(
          4.5,
        );
      });
    }
  }
});

function contrast(first: string, second: string): number {
  const luminance = (hex: string) => {
    const channels = [1, 3, 5].map(
      (position) => parseInt(hex.slice(position, position + 2), 16) / 255,
    );
    const linear = channels.map((channel) =>
      channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
    );
    return linear[0] * 0.2126 + linear[1] * 0.7152 + linear[2] * 0.0722;
  };
  const a = luminance(first),
    b = luminance(second);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
