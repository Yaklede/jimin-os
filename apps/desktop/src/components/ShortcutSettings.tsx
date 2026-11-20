import { useId, useState, type KeyboardEvent } from "react";
import { shortcutCopy as c } from "../copy/shortcuts";
import {
  appleKeyboard,
  defaultShortcuts,
  reservedShortcut,
  sameShortcut,
  shortcutActions,
  shortcutFromEvent,
  shortcutLabel,
  type ShortcutAction,
  type Shortcuts,
} from "../keyboardShortcuts";
import "./shortcutSettings.css";

export function ShortcutSettings({
  shortcuts,
  onSave,
}: {
  shortcuts: Shortcuts;
  onSave(value: Shortcuts): boolean;
}) {
  const apple = appleKeyboard();
  const id = useId();
  const [recording, setRecording] = useState<ShortcutAction>();
  const [error, setError] = useState("");
  const [note, setNote] = useState("");
  const prompt = c.prompt(apple ? "⌘" : "Ctrl");

  function record(
    action: ShortcutAction,
    event: KeyboardEvent<HTMLButtonElement>,
  ) {
    if (recording !== action || event.key === "Tab") return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      setRecording(undefined);
      setError("");
      return;
    }
    const next = shortcutFromEvent(event.nativeEvent, apple);
    if (!next) {
      if (event.key.length === 1 && !event.nativeEvent.isComposing)
        setError(prompt);
      return;
    }
    if (reservedShortcut(next)) {
      setError(c.reserved);
      return;
    }
    const conflict = shortcutActions.find(
      (other) => other !== action && sameShortcut(shortcuts[other], next),
    );
    if (conflict) {
      setError(c.duplicate(c.actions[conflict]));
      return;
    }
    if (!onSave({ ...shortcuts, [action]: next })) {
      setError(c.saveProblem);
      return;
    }
    setError("");
    setRecording(undefined);
    setNote(c.saved);
  }

  return (
    <section className="settings-shortcuts" aria-labelledby={id}>
      <div className="settings-list__section-heading">
        <h2 id={id}>{c.title}</h2>
        <p>{c.help}</p>
      </div>
      <ul>
        {shortcutActions.map((action) => {
          const binding = shortcutLabel(shortcuts[action], apple);
          return (
            <li key={action}>
              <span>{c.actions[action]}</span>
              <button
                type="button"
                className="secondary-button focus-visible-control"
                aria-label={c.edit(c.actions[action], binding)}
                aria-pressed={recording === action}
                aria-describedby={
                  recording === action ? `${id}-feedback` : undefined
                }
                data-shortcut-recorder={recording === action || undefined}
                onClick={() => {
                  setError("");
                  setNote("");
                  setRecording(recording === action ? undefined : action);
                }}
                onBlur={() => {
                  setRecording((current) =>
                    current === action ? undefined : current,
                  );
                  setError("");
                }}
                onKeyDown={(event) => record(action, event)}
              >
                {recording === action ? c.record : <kbd>{binding}</kbd>}
              </button>
            </li>
          );
        })}
      </ul>
      <div className="settings-shortcuts__footer">
        <p id={`${id}-feedback`} role="status">
          {error || (recording ? prompt : note)}
        </p>
        <button
          type="button"
          className="secondary-button focus-visible-control"
          onClick={() => {
            setRecording(undefined);
            const ok = onSave(defaultShortcuts);
            setError(ok ? "" : c.saveProblem);
            setNote(ok ? c.restored : "");
          }}
        >
          {c.reset}
        </button>
      </div>
    </section>
  );
}
