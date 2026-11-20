import { describe, expect, it } from "vitest";
import {
  defaultShortcuts,
  reservedShortcut,
  sameShortcut,
  shortcutFromEvent,
  shortcutLabel,
} from "./keyboardShortcuts";

function key(overrides: Partial<KeyboardEvent> = {}) {
  return {
    code: "KeyK",
    key: "k",
    metaKey: true,
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    isComposing: false,
    repeat: false,
    getModifierState: () => false,
    ...overrides,
  } as KeyboardEvent;
}

describe("keyboard shortcuts", () => {
  it("accepts Command on macOS and Control elsewhere", () => {
    expect(shortcutFromEvent(key(), true)).toEqual(defaultShortcuts.ask);
    expect(shortcutFromEvent(key(), false)).toBeUndefined();
    expect(
      shortcutFromEvent(key({ ctrlKey: true, metaKey: false }), false),
    ).toEqual(defaultShortcuts.ask);
    expect(shortcutLabel(defaultShortcuts.ask, true, true)).toBe("Meta+K");
    expect(shortcutLabel(defaultShortcuts.ask, false, true)).toBe("Control+K");
  });

  it("matches the physical digit even if Option produces a different character", () => {
    expect(
      shortcutFromEvent(key({ code: "Digit1", key: "¡", altKey: true }), true),
    ).toEqual(defaultShortcuts.calendar);
    expect(
      sameShortcut(defaultShortcuts.calendar, defaultShortcuts.scheduled),
    ).toBe(false);
  });

  it("ignores repeats, composing and AltGr; preserves common browser combinations", () => {
    expect(shortcutFromEvent(key({ repeat: true }), true)).toBeUndefined();
    expect(shortcutFromEvent(key({ isComposing: true }), true)).toBeUndefined();
    expect(
      shortcutFromEvent(key({ getModifierState: () => true }), true),
    ).toBeUndefined();
    expect(reservedShortcut({ code: "KeyL", alt: false, shift: false })).toBe(
      true,
    );
    expect(reservedShortcut(defaultShortcuts.ask)).toBe(false);
  });
});
