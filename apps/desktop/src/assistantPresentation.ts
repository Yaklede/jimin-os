import { type HomeSnapshot } from "./api/home";
import { type Task } from "./api/planning";
import { type Project } from "./api/projects";

export type AssistantPresentation =
  | {
      kind: "tasks";
      title: string;
      summary: string;
      items: Task[];
      highlightedTaskId: string | undefined;
    }
  | {
      kind: "schedule";
      title: string;
      summary: string;
      items: HomeSnapshot["schedule"];
    }
  | {
      kind: "projects";
      title: string;
      summary: string;
      items: Project[];
    }
  | {
      kind: "summary";
      title: string;
      summary: string;
    };

const TASK_TERMS = ["일감", "할 일", "할일", "업무", "태스크", "task"];
const SCHEDULE_TERMS = ["일정", "약속", "회의", "캘린더", "calendar"];
const PROJECT_TERMS = ["프로젝트", "project"];
const QUERY_STOP_WORDS = new Set([
  "내",
  "나의",
  "내가",
  "관련",
  "일감",
  "할",
  "일",
  "할일",
  "업무",
  "태스크",
  "task",
  "일정",
  "약속",
  "회의",
  "캘린더",
  "calendar",
  "프로젝트",
  "project",
  "찾아줘",
  "찾아",
  "보여줘",
  "보여",
  "알려줘",
  "알려",
  "정리해줘",
  "정리",
  "확인해줘",
  "확인",
  "해줘",
  "줘",
]);

export function deriveAssistantPresentation(
  request: string,
  response: string,
  snapshot: HomeSnapshot | undefined,
  projects: Project[],
): AssistantPresentation {
  const normalized = normalize(request);
  const summary = response.trim() || "요청한 내용을 확인했어요.";
  const resultSummary = conciseLead(summary);

  if (containsAny(normalized, TASK_TERMS)) {
    const items = rankMatches(request, snapshot?.tasks ?? [], (task) =>
      [task.title, task.notes].filter(Boolean).join(" "),
    );
    return {
      kind: "tasks",
      title: items.length
        ? `관련 일감 ${items.length}개`
        : "관련 일감을 찾지 못했어요",
      summary: resultSummary,
      items,
      highlightedTaskId: items[0]?.id,
    };
  }

  if (containsAny(normalized, SCHEDULE_TERMS)) {
    return {
      kind: "schedule",
      title: "오늘 일정",
      summary: resultSummary,
      items: snapshot?.schedule ?? [],
    };
  }

  if (containsAny(normalized, PROJECT_TERMS)) {
    const items = rankMatches(request, projects, (project) =>
      [project.title, project.objective, project.nextAction]
        .filter(Boolean)
        .join(" "),
    );
    return {
      kind: "projects",
      title: items.length
        ? `관련 프로젝트 ${items.length}개`
        : "관련 프로젝트를 찾지 못했어요",
      summary: resultSummary,
      items,
    };
  }

  return { kind: "summary", title: "요청 결과", summary };
}

function conciseLead(value: string): string {
  const firstLine = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
  const plain = (firstLine || value)
    .replace(/^[#>*\-\s]+/, "")
    .replace(/[*_`]/g, "")
    .trim();
  if (plain.length <= 240) return plain;
  return `${plain.slice(0, 237).trimEnd()}…`;
}

function rankMatches<T>(
  request: string,
  items: T[],
  searchableText: (item: T) => string,
): T[] {
  const queryTokens = tokens(request);
  if (!queryTokens.length) return items;

  return items
    .map((item, index) => ({
      item,
      index,
      score: queryTokens.reduce((score, token) => {
        const text = normalize(searchableText(item));
        return score + (text.includes(token) ? token.length : 0);
      }, 0),
    }))
    .filter(({ score }) => score > 0)
    .sort((left, right) => right.score - left.score || left.index - right.index)
    .map(({ item }) => item);
}

function tokens(value: string): string[] {
  return normalize(value)
    .split(/\s+/)
    .map((token) => token.replace(/[은는이가을를에도와과로부터까지]/g, ""))
    .filter((token) => token.length >= 2 && !QUERY_STOP_WORDS.has(token));
}

function containsAny(value: string, terms: string[]): boolean {
  return terms.some((term) => value.includes(term));
}

function normalize(value: string): string {
  return value
    .normalize("NFKC")
    .toLocaleLowerCase("ko-KR")
    .replace(/[^0-9a-z가-힣]+/g, " ")
    .trim();
}
