import { useId, useRef } from "react";
import { CalendarDays } from "lucide-react";

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
  const dateInputRef = useRef<HTMLInputElement>(null);
  const labelId = `${id || generatedId}-label`;
  const [date = "", time = ""] = value.split("T");
  const [hour = "", minute = ""] = time.split(":");

  function changeDate(nextDate: string) {
    if (!nextDate) {
      onChange("");
      return;
    }
    onChange(combineLocalDateTime(nextDate, time || nextQuarterHour(now)));
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
            className="deadline-picker__calendar"
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
          {showPresets && (
            <div className="deadline-picker__date-presets">
              <button
                type="button"
                disabled={disabled}
                onClick={() => changeDate(seoulDatePart(now))}
              >
                {deadlinePickerCopy.today}
              </button>
              <button
                type="button"
                disabled={disabled}
                onClick={() =>
                  changeDate(seoulDatePart(new Date(now.getTime() + 86400000)))
                }
              >
                {deadlinePickerCopy.tomorrow}
              </button>
            </div>
          )}
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
                    now,
                  ),
                )
              }
            >
              <option value="">{deadlinePickerCopy.hour}</option>
              {Array.from({ length: 24 }, (_, index) => (
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
                    now,
                  ),
                )
              }
            >
              <option value="">{deadlinePickerCopy.minute}</option>
              {Array.from({ length: 60 }, (_, index) => (
                <option key={index} value={pad(index)}>
                  {pad(index)}
                  {deadlinePickerCopy.minute}
                </option>
              ))}
            </select>
          </label>
        </div>
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
