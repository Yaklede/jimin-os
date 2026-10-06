import { CheckCircle2, Circle } from "lucide-react";
import { useState } from "react";

/** Selecting a task never changes its status. Completion requires a named action. */
export function TaskSelectionControl({
  title,
  className,
  disabled,
  busy,
  hint,
  onComplete,
}: {
  title: string;
  className: string;
  disabled?: boolean;
  busy?: boolean;
  hint?: string;
  onComplete(): void | Promise<void>;
}) {
  const [selected, setSelected] = useState(false);
  return (
    <div className="task-selection-control">
      <button
        className={className}
        type="button"
        role="checkbox"
        aria-checked={selected}
        aria-label={`${title} 선택`}
        title={hint}
        disabled={disabled}
        onClick={() => setSelected((value) => !value)}
      >
        {busy ? (
          <span className="button-spinner" aria-hidden="true" />
        ) : selected ? (
          <CheckCircle2 aria-hidden="true" />
        ) : (
          <Circle aria-hidden="true" />
        )}
      </button>
      {selected && (
        <button
          className="task-selection-control__finish focus-visible-control"
          type="button"
          disabled={disabled}
          aria-label={`${title} 완료하기`}
          onClick={() => void onComplete()}
        >
          완료 처리하기
        </button>
      )}
    </div>
  );
}
