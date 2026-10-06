import { Check, Moon, Palette, Sun, X } from "lucide-react";
import { type CSSProperties, useEffect, useId, useRef, useState } from "react";
import { registerMobileBackHandler } from "../mobileBack";

import {
  appearanceColors,
  appearancePalettes,
  applyAppearance,
  currentAppearance,
  persistAppearance,
  type AppearancePreferences,
} from "../appearance-preferences";
import { copy } from "../copy";
import { RepresentativeImageControl } from "./RepresentativeImageControl";

export function AppearanceControl() {
  const popoverId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const [preferences, setPreferences] = useState(currentAppearance);
  const [saveFailed, setSaveFailed] = useState(false);

  useEffect(() => applyAppearance(preferences), [preferences]);
  useEffect(
    () =>
      registerMobileBackHandler(() => {
        const panel = panelRef.current;
        if (!panel?.matches(":popover-open")) return false;
        panel.hidePopover();
        return true;
      }, 100),
    [],
  );

  function selectPreferences(next: AppearancePreferences) {
    setPreferences(next);
    setSaveFailed(!persistAppearance(next));
  }

  return (
    <div className="appearance-control">
      <button
        type="button"
        className="appearance-control__trigger focus-visible-control"
        aria-label={copy.appearance.open}
        aria-haspopup="dialog"
        popoverTarget={popoverId}
      >
        <Palette aria-hidden="true" />
      </button>
      <div
        id={popoverId}
        ref={panelRef}
        popover="auto"
        className="appearance-panel"
        role="dialog"
        aria-label={copy.appearance.title}
      >
        <header className="appearance-panel__heading">
          <strong>{copy.appearance.title}</strong>
          <button
            type="button"
            className="appearance-panel__close focus-visible-control"
            aria-label={copy.appearance.close}
            popoverTarget={popoverId}
            popoverTargetAction="hide"
          >
            <X aria-hidden="true" />
          </button>
        </header>
        <div
          role="group"
          aria-label={copy.appearance.mode}
          className="appearance-panel__section"
        >
          <span>{copy.appearance.mode}</span>
          <div className="appearance-panel__modes">
            {(["light", "dark"] as const).map((mode) => (
              <button
                type="button"
                key={mode}
                className="appearance-panel__mode focus-visible-control"
                aria-pressed={preferences.mode === mode}
                onClick={() => selectPreferences({ ...preferences, mode })}
              >
                {mode === "light" ? (
                  <Sun aria-hidden="true" />
                ) : (
                  <Moon aria-hidden="true" />
                )}
                {copy.appearance[mode]}
                {preferences.mode === mode && <Check aria-hidden="true" />}
              </button>
            ))}
          </div>
        </div>
        <div
          role="group"
          aria-label={copy.appearance.color}
          className="appearance-panel__section"
        >
          <span>{copy.appearance.color}</span>
          <div className="appearance-panel__colors">
            {appearanceColors.map((color) => (
              <button
                type="button"
                key={color}
                className="appearance-panel__color focus-visible-control"
                aria-pressed={preferences.color === color}
                onClick={() => selectPreferences({ ...preferences, color })}
                style={
                  {
                    "--swatch":
                      appearancePalettes[color][preferences.mode].accent,
                    "--on-swatch":
                      appearancePalettes[color][preferences.mode].onAccent,
                  } as CSSProperties
                }
              >
                <span className="appearance-panel__swatch" aria-hidden="true">
                  {preferences.color === color && <Check />}
                </span>
                <span>{copy.appearance.colors[color]}</span>
              </button>
            ))}
          </div>
        </div>
        <RepresentativeImageControl />
        <p
          className="appearance-panel__help"
          role={saveFailed ? "status" : undefined}
        >
          {saveFailed ? copy.appearance.saveFailed : copy.appearance.saved}
        </p>
      </div>
    </div>
  );
}
