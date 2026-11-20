export const shortcutActions = ["ask", "calendar", "scheduled"] as const;
export type ShortcutAction = (typeof shortcutActions)[number];
export type Shortcut = { code: string; alt: boolean; shift: boolean };
export type Shortcuts = Record<ShortcutAction, Shortcut>;
const storageKey = "jimin-os.keyboard-shortcuts.v1";

export const defaultShortcuts: Shortcuts = {
  ask: { code: "KeyK", alt: false, shift: false },
  calendar: { code: "Digit1", alt: true, shift: false },
  scheduled: { code: "Digit2", alt: true, shift: false },
};

export function appleKeyboard() {
  return (
    typeof navigator !== "undefined" &&
    /Mac|iPhone|iPad|iPod/i.test(navigator.platform)
  );
}

function validShortcut(value: unknown): value is Shortcut {
  if (!value || typeof value !== "object") return false;
  const s = value as Partial<Shortcut>;
  return (
    typeof s.code === "string" &&
    /^(Key[A-Z]|Digit[0-9])$/.test(s.code) &&
    typeof s.alt === "boolean" &&
    typeof s.shift === "boolean"
  );
}

export const sameShortcut = (a: Shortcut, b: Shortcut) =>
  a.code === b.code && a.alt === b.alt && a.shift === b.shift;

export function reservedShortcut(shortcut: Shortcut) {
  // Do not replace common editing, browser, or operating system shortcuts.
  return (
    !shortcut.alt &&
    !shortcut.shift &&
    /^(Digit[0-9]|Key[ACFLNOPQRSTVWXZ])$/.test(shortcut.code)
  );
}

export function readShortcuts(): Shortcuts {
  try {
    const saved = JSON.parse(window.localStorage.getItem(storageKey) ?? "null");
    if (
      saved &&
      shortcutActions.every(
        (action) =>
          validShortcut(saved[action]) && !reservedShortcut(saved[action]),
      ) &&
      !shortcutActions.some((action, i) =>
        shortcutActions
          .slice(i + 1)
          .some((other) => sameShortcut(saved[action], saved[other])),
      )
    ) {
      return {
        ask: saved.ask,
        calendar: saved.calendar,
        scheduled: saved.scheduled,
      };
    }
  } catch {
    // The default bindings still work when local storage is unavailable.
  }
  return defaultShortcuts;
}

export function saveShortcuts(shortcuts: Shortcuts) {
  try {
    window.localStorage.setItem(storageKey, JSON.stringify(shortcuts));
    return true;
  } catch {
    return false;
  }
}

export function shortcutFromEvent(event: KeyboardEvent, apple: boolean) {
  if (
    event.isComposing ||
    event.key === "Process" ||
    event.repeat ||
    event.getModifierState("AltGraph") ||
    (apple ? !event.metaKey || event.ctrlKey : !event.ctrlKey || event.metaKey)
  ) {
    return undefined;
  }
  const shortcut = {
    code: event.code,
    alt: event.altKey,
    shift: event.shiftKey,
  };
  return validShortcut(shortcut) ? shortcut : undefined;
}

export function shortcutLabel(
  shortcut: Shortcut,
  apple: boolean,
  aria = false,
) {
  const key = shortcut.code.replace(/^(Key|Digit)/, "");
  const names = [
    apple ? (aria ? "Meta" : "⌘") : "Ctrl",
    ...(shortcut.alt ? [apple && !aria ? "⌥" : "Alt"] : []),
    ...(shortcut.shift ? [apple && !aria ? "⇧" : "Shift"] : []),
    key,
  ];
  if (aria && !apple) names[0] = "Control";
  return names.join(apple && !aria ? "" : "+");
}

export function editableShortcutTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    Boolean(
      target.closest(
        'input, textarea, select, [contenteditable]:not([contenteditable="false"]), [role="textbox"]',
      ),
    )
  );
}
