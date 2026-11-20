import { useId, useRef, type ReactNode } from "react";
import { CalendarDays, Check } from "lucide-react";

import { deadlinePickerCopy } from "../copy/deadlinePicker";

const SEOUL_OFFSET_MILLIS = 9 * 60 * 60 * 1_000;
const LOCAL_DATE_TIME_PATTERN = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/;
const HOURS = Array.from({ length: 24 }, (_, index) => index);
const MINUTES = Array.from({ length: 60 }, (_, index) => index);
const TIME_PRESETS = [
  { label: deadlinePickerCopy.todayTen, day: 0, hour: 10, minute: 0 },
  { label: deadlinePickerCopy.todayThree, day: 0, hour: 15, minute: 0 },
  { label: deadlinePickerCopy.todaySix, day: 0, hour: 18, minute: 0 },
  { label: deadlinePickerCopy.todayEnd, day: 0, hour: 23, minute: 45 },
  { label: deadlinePickerCopy.tomorrowSix, day: 1, hour: 18, minute: 0 },
];
const DATE_PRESETS = [
  { label: deadlinePickerCopy.today, day: 0 },
  { label: deadlinePickerCopy.tomorrow, day: 1 },
];

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
  collapsePresets?: boolean;
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
  collapsePresets = false,
  allowClear = true,
  now,
  className,
}: DeadlinePickerProps) {
  const generatedId = useId();
  const dateInputRef = useRef<HTMLInputElement>(null);
  const labelId = `${id || generatedId}-label`;
  const [date = "", time = ""] = value.split("T");
  const [hour = "", minute = ""] = time.split(":");
  const presetTime = now ?? new Date();

  function currentTime() {
    return now ?? new Date();
  }

  function changeDate(nextDate: string) {
    if (!nextDate) {
      onChange("");
      return;
    }
    onChange(
      combineLocalDateTime(nextDate, time || nextQuarterHour(currentTime())),
    );
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
        <div className="deadline-picker__date">
          <label>
            <span>{deadlinePickerCopy.date}</span>
            <input
              ref={dateInputRef}
              id={`${id}-date`}
              type="date"
              value={date}
              disabled={disabled}
              required={required}
              aria-invalid={invalid}
              aria-label={`${label} ${deadlinePickerCopy.date}`}
              onChange={(event) => changeDate(event.currentTarget.value)}
            />
          </label>
          <button
            type="button"
            className="deadline-picker__calendar focus-visible-control"
            disabled={disabled}
            aria-label={`${label} ${deadlinePickerCopy.openCalendar}`}
            onClick={() => {
              const input = dateInputRef.current;
              if (!input) return;
              try {
                if (input.showPicker) input.showPicker();
                else input.focus();
              } catch {
                input.focus();
              }
            }}
          >
            <CalendarDays aria-hidden="true" />
          </button>
        </div>
        <div className="deadline-picker__time">
          <label>
            <span>{deadlinePickerCopy.time}</span>
            <select
              id={`${id}-time`}
              aria-label={`${label} ${deadlinePickerCopy.hour}`}
              value={hour}
              disabled={disabled}
              required={required}
              aria-invalid={invalid}
              onChange={(event) =>
                onChange(
                  selectLocalTimePart(
                    value,
                    "hour",
                    event.currentTarget.value,
                    currentTime(),
                  ),
                )
              }
            >
              <option value="">{deadlinePickerCopy.hour}</option>
              {HOURS.map((index) => (
                <option key={index} value={pad(index)}>
                  {pad(index)}
                  {deadlinePickerCopy.hour}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span className="deadline-picker__minute-label">
              {deadlinePickerCopy.minute}
            </span>
            <select
              id={`${id}-minute`}
              aria-label={`${label} ${deadlinePickerCopy.minute}`}
              value={minute}
              disabled={disabled}
              required={required}
              aria-invalid={invalid}
              onChange={(event) =>
                onChange(
                  selectLocalTimePart(
                    value,
                    "minute",
                    event.currentTarget.value,
                    currentTime(),
                  ),
                )
              }
            >
              <option value="">{deadlinePickerCopy.minute}</option>
              {MINUTES.map(minuteOption)}
            </select>
          </label>
        </div>
      </div>
      {showPresets && (
        <DeadlinePresets collapsed={collapsePresets}>
          <div
            className="deadline-picker__date-presets"
            role="group"
            aria-label={deadlinePickerCopy.datePresets}
          >
            {DATE_PRESETS.map(({ label, day }) => (
              <button
                key={label}
                type="button"
                className="focus-visible-control"
                disabled={disabled}
                onClick={() => changeDate(seoulDatePart(currentTime(), day))}
              >
                {label}
              </button>
            ))}
            {allowClear && !required && (
              <button
                type="button"
                className="focus-visible-control"
                disabled={disabled || !value}
                onClick={() => onChange("")}
              >
                {deadlinePickerCopy.clear}
              </button>
            )}
          </div>
          <div
            className="deadline-picker__presets"
            role="group"
            aria-label={deadlinePickerCopy.presets}
          >
            {TIME_PRESETS.map(({ label, day, hour, minute }) => (
              <button
                key={label}
                type="button"
                className="focus-visible-control"
                disabled={disabled}
                aria-pressed={
                  value === seoulPreset(presetTime, day, hour, minute)
                }
                onClick={() =>
                  onChange(seoulPreset(currentTime(), day, hour, minute))
                }
              >
                <Check
                  className="deadline-picker__preset-check"
                  aria-hidden="true"
                />
                <span>{label}</span>
              </button>
            ))}
          </div>
        </DeadlinePresets>
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

function DeadlinePresets({
  collapsed,
  children,
}: {
  collapsed: boolean;
  children: ReactNode;
}) {
  if (!collapsed) return <>{children}</>;
  return (
    <details className="deadline-picker__shortcuts">
      <summary className="focus-visible-control">
        {deadlinePickerCopy.presets}
      </summary>
      {children}
    </details>
  );
}

function minuteOption(minute: number) {
  const value = pad(minute);
  return (
    <option key={value} value={value}>
      {value}
      {deadlinePickerCopy.minute}
    </option>
  );
}

export function selectLocalTimePart(
  value: string,
  part: "hour" | "minute",
  selected: string,
  now = new Date(),
): string {
  if (!selected) return "";
  if (
    !/^\d{2}$/.test(selected) ||
    Number(selected) > (part === "hour" ? 23 : 59)
  )
    return value;
  const [date, time = ""] = value.split("T");
  const [hour = "09", minute = "00"] = time.split(":");
  return combineLocalDateTime(
    date || seoulDatePart(now),
    `${part === "hour" ? selected : hour || "09"}:${part === "minute" ? selected : minute || "00"}`,
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

function seoulDatePart(now: Date, dayOffset = 0): string {
  const seoul = new Date(now.getTime() + SEOUL_OFFSET_MILLIS);
  seoul.setUTCDate(seoul.getUTCDate() + dayOffset);
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
  return `${seoulDatePart(now, dayOffset)}T${pad(hour)}:${pad(minute)}`;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}
