// Keep this pure formatter aligned with storage::planning's reviewed formatter.
// Cross-language fixtures assert the same body; provider mention markup is added
// separately and is not represented as a raw Google user ID in the preview.
export type AssignmentPreviewInput = {
  projectTitle: string;
  title: string;
  content: string;
  assigneeName?: string | null;
  priority: number;
  dueAt: string | null;
  referenceLinks: string[];
};

const length = (value: string) => Array.from(value).length;
const take = (value: string, maximum: number) =>
  Array.from(value).slice(0, maximum).join("");

function label(value: string, maximum: number, fallback: string) {
  const compact = value
    .replace(/보낸 사람 정보 없음[:：]?|보낸사람 정보 없음:?/g, "")
    .split(/\s+/u)
    .filter((part) => part && !/^(https?:\/\/|@)/.test(part))
    .join(" ");
  return compact
    ? take(compact, maximum) + (length(compact) > maximum ? "…" : "")
    : fallback;
}

function deadline(value: string | null) {
  if (!value || !Number.isFinite(Date.parse(value))) return "정하지 않음";
  const parts = new Intl.DateTimeFormat("en-CA", {
    timeZone: "Asia/Seoul",
    year: "numeric",
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  }).formatToParts(new Date(value));
  const part = (type: Intl.DateTimeFormatPartTypes) =>
    parts.find((entry) => entry.type === type)?.value ?? "";
  return `${part("year")}년 ${Number(part("month"))}월 ${Number(part("day"))}일 ${part("hour")}:${part("minute")}`;
}

export function reviewedAssignmentPreview(input: AssignmentPreviewInput) {
  let message = [
    input.assigneeName ? "새 할 일이 배정됐어요." : "새 할 일이 등록됐어요.",
    "",
    `프로젝트: ${label(input.projectTitle, 120, "프로젝트")}`,
    `할 일: ${label(input.title, 200, "새 할 일")}`,
    `담당자: ${input.assigneeName || "미정"}`,
    `마감: ${deadline(input.dueAt)}`,
    `우선순위: ${input.priority <= 1 ? "일반" : input.priority === 2 ? "우선 처리" : "가장 먼저"}`,
  ].join("\n");
  const uniqueLinks = [
    ...new Set(input.referenceLinks.map((link) => link.trim())),
  ]
    .filter((link) => {
      const authority = link.match(/^https?:\/\/([^/?#]+)/)?.[1];
      return (
        length(link) <= 2048 &&
        !/[\p{Cc}]/u.test(link) &&
        Boolean(authority && !/[@\s]/u.test(authority))
      );
    })
    .slice(0, 20);
  let references = "관련 자료";
  let referenceCount = 0;
  for (const link of uniqueLinks.slice(0, 3)) {
    const line = `\n- ${link}`;
    if (length(references + line) > 420) break;
    references += line;
    referenceCount++;
  }
  if (!referenceCount) references = "";
  const available = Math.max(
    0,
    1600 - (references ? length(references) + 2 : 0) - length(message) - 8,
  );
  const content = input.content.replace(/\r\n?/g, "\n").trim();
  const truncated =
    Boolean(content) && (available < 24 || length(content) > available);
  if (content && available >= 24) {
    message +=
      "\n\n업무 내용\n" +
      (truncated ? take(content, available - 1).trimEnd() + "…" : content);
  }
  if (references && length(message + references) + 2 <= 1600)
    message += `\n\n${references}`;
  return { message, truncated };
}
