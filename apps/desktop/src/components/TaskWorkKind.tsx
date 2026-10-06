import type { TaskWorkKind } from "../api/planning";
import "./taskWorkKind.css";

export const taskWorkKindCopy = {
  label: "업무 유형",
  general: "일반 업무",
  verification: "확인 업무",
  development: "개발 업무",
  hint: "확인 업무는 완료할 때 확인 결과를 남길 수 있어요.",
};

export function taskWorkKindLabel(value?: TaskWorkKind) {
  return taskWorkKindCopy[value ?? "general"];
}

export function TaskWorkKindSelect({
  value,
  disabled,
  onChange,
  compact = false,
}: {
  value: TaskWorkKind;
  disabled?: boolean;
  onChange(value: TaskWorkKind): void;
  compact?: boolean;
}) {
  return (
    <label
      className={`task-work-kind-field${compact ? " task-work-kind-field--compact" : " planning-editor__field"}`}
    >
      <span>{taskWorkKindCopy.label}</span>
      <select
        aria-label={taskWorkKindCopy.label}
        value={value}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value as TaskWorkKind)}
      >
        <option value="verification">{taskWorkKindCopy.verification}</option>
        <option value="development">{taskWorkKindCopy.development}</option>
        <option value="general">{taskWorkKindCopy.general}</option>
      </select>
      {!compact && <small>{taskWorkKindCopy.hint}</small>}
    </label>
  );
}

export function TaskWorkKindBadge({ kind }: { kind?: TaskWorkKind }) {
  if (!kind || kind === "general") return null;
  return <small className="task-work-kind">{taskWorkKindLabel(kind)}</small>;
}
