import { useId } from "react";

import { deadlinePickerCopy } from "../copy/deadlinePicker";

const SEOUL_OFFSET_MILLIS = 9 * 60 * 60 * 1_000;
const LOCAL_DATE_TIME_PATTERN = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/;

type DeadlinePickerProps = {
  id: string;
  label: string;
  value: string;
  onChange(value: string): void;
  disabled?: boolean;
  required?: boolean;
  invalid?: boolean;
  describedBy?: string;
  showPresets?: boolean;
  allowClear?: boolean;
  now?: Date;
  className?: string;
};

export function DeadlinePicker({
  id,
  label,
  value,
  onChange,
  disabled = false,
  required = false,
  invalid = false,
  describedBy,
  showPresets = false,
  allowClear = true,
  now = new Date(),
  className,
}: DeadlinePickerProps) {
  const generatedId = useId();
  const labelId = `${id || generatedId}-label`;
  const [date = "", time = ""] = value.split("T");

  function changeDate(nextDate: string) {
    if (!nextDate) {
      onChange("");
      return;
    }
    onChange(combineLocalDateTime(nextDate, time || nextQuarterHour(now)));
  }

  function changeTime(nextTime: string) {
    if (!nextTime) {
      onChange("");
      return;
    }
    onChange(combineLocalDateTime(date || seoulDatePart(now), nextTime));
  }

  return (
    <div
      className={["deadline-picker", className].filter(Boolean).join(" ")}
      data-invalid={invalid || undefined}
    >
      <span className="deadline-picker__label" id={labelId}>
        {label}
      </span>
      <div
        className="deadline-picker__inputs"
        role="group"
        aria-labelledby={labelId}
        aria-describedby={describedBy}
      >
        <label>
          <span>{deadlinePickerCopy.date}</span>
          <input
            id={`${id}-date`}
            type="date"
            value={date}
            disabled={disabled}
            required={required}
            aria-invalid={invalid}
            onChange={(event) => changeDate(event.currentTarget.value)}
          />
        </label>
        <label>
          <span>{deadlinePickerCopy.time}</span>
          <input
            id={`${id}-time`}
            type="time"
            step={15 * 60}
            value={time}
            disabled={disabled}
            required={required}
            aria-invalid={invalid}
            onChange={(event) => changeTime(event.currentTarget.value)}
          />
        </label>
      </div>
      {showPresets && (
        <div
          className="deadline-picker__presets"
          aria-label={deadlinePickerCopy.presets}
        >
          <button
            type="button"
            disabled={disabled}
            onClick={() => onChange(seoulPreset(now, 0, 18, 0))}
          >
            {deadlinePickerCopy.todaySix}
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={() => onChange(seoulPreset(now, 0, 23, 45))}
          >
            {deadlinePickerCopy.todayEnd}
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={() => onChange(seoulPreset(now, 1, 18, 0))}
          >
            {deadlinePickerCopy.tomorrowSix}
          </button>
          {allowClear && (
            <button
              type="button"
              disabled={disabled || !value}
              onClick={() => onChange("")}
            >
              {deadlinePickerCopy.clear}
            </button>
          )}
        </div>
      )}
      <p
        className="deadline-picker__preview"
        aria-live="polite"
        data-empty={!isCompleteLocalDateTime(value) || undefined}
      >
        {formatSeoulDateTimePreview(value)}
      </p>
    </div>
  );
}

export function combineLocalDateTime(date: string, time: string): string {
  return date && time ? `${date}T${time.slice(0, 5)}` : "";
}

export function isCompleteLocalDateTime(value: string): boolean {
  return parseSeoulLocalDateTime(value) !== undefined;
}

export function seoulLocalDateTimeToIso(value: string): string | undefined {
  const parsed = parseSeoulLocalDateTime(value);
  if (!parsed) return undefined;
  return new Date(
    Date.UTC(
      parsed.year,
      parsed.month - 1,
      parsed.day,
      parsed.hour,
      parsed.minute,
    ) - SEOUL_OFFSET_MILLIS,
  ).toISOString();
}

export type OptionalSeoulDateTimeResolution =
  | { valid: true; value: string | undefined }
  | { valid: false; value: undefined };

export function resolveOptionalSeoulDateTime(
  value: string,
): OptionalSeoulDateTimeResolution {
  if (!value) return { valid: true, value: undefined };
  const iso = seoulLocalDateTimeToIso(value);
  return iso ? { valid: true, value: iso } : { valid: false, value: undefined };
}

export function isoToSeoulLocalDateTime(value: string | null): string {
  if (!value) return "";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "";
  const seoul = new Date(date.getTime() + SEOUL_OFFSET_MILLIS);
  return [
    seoul.getUTCFullYear(),
    "-",
    pad(seoul.getUTCMonth() + 1),
    "-",
    pad(seoul.getUTCDate()),
    "T",
    pad(seoul.getUTCHours()),
    ":",
    pad(seoul.getUTCMinutes()),
  ].join("");
}

export function formatSeoulDateTimePreview(value: string): string {
  const iso = seoulLocalDateTimeToIso(value);
  if (!iso) return deadlinePickerCopy.empty;
  const label = new Intl.DateTimeFormat("ko-KR", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "long",
    day: "numeric",
    weekday: "short",
    hour: "numeric",
    minute: "2-digit",
  }).format(new Date(iso));
  return `${label} · ${deadlinePickerCopy.timezone}`;
}

function parseSeoulLocalDateTime(value: string) {
  const match = LOCAL_DATE_TIME_PATTERN.exec(value);
  if (!match) return undefined;
  const [, yearText, monthText, dayText, hourText, minuteText] = match;
  const year = Number(yearText);
  const month = Number(monthText);
  const day = Number(dayText);
  const hour = Number(hourText);
  const minute = Number(minuteText);
  if (
    month < 1 ||
    month > 12 ||
    day < 1 ||
    day > 31 ||
    hour < 0 ||
    hour > 23 ||
    minute < 0 ||
    minute > 59
  ) {
    return undefined;
  }
  const check = new Date(Date.UTC(year, month - 1, day, hour, minute));
  if (
    check.getUTCFullYear() !== year ||
    check.getUTCMonth() !== month - 1 ||
    check.getUTCDate() !== day
  ) {
    return undefined;
  }
  return { year, month, day, hour, minute };
}

function nextQuarterHour(now: Date): string {
  const seoul = new Date(now.getTime() + SEOUL_OFFSET_MILLIS);
  const minutes = seoul.getUTCMinutes();
  seoul.setUTCMinutes(Math.ceil((minutes + 1) / 15) * 15, 0, 0);
  return `${pad(seoul.getUTCHours())}:${pad(seoul.getUTCMinutes())}`;
}

function seoulDatePart(now: Date): string {
  const seoul = new Date(now.getTime() + SEOUL_OFFSET_MILLIS);
  return `${seoul.getUTCFullYear()}-${pad(seoul.getUTCMonth() + 1)}-${pad(
    seoul.getUTCDate(),
  )}`;
}

function seoulPreset(
  now: Date,
  dayOffset: number,
  hour: number,
  minute: number,
): string {
  const seoul = new Date(now.getTime() + SEOUL_OFFSET_MILLIS);
  seoul.setUTCDate(seoul.getUTCDate() + dayOffset);
  return `${seoul.getUTCFullYear()}-${pad(seoul.getUTCMonth() + 1)}-${pad(
    seoul.getUTCDate(),
  )}T${pad(hour)}:${pad(minute)}`;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}
