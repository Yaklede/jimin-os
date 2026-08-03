import { PlanningRequestError } from "./planning";

export type ReportStatus = "draft" | "finalized" | "archived" | "failed";

export interface ReportMetric {
  key: string;
  label: string;
  value: number | null;
}

export interface ProjectWeeklyReportContent {
  kind: "project_weekly";
  period: { start: string; end: string };
  summary: string;
  metrics: ReportMetric[];
  focus: string[];
  evidence: Array<{
    type: string;
    workspaceId: string;
    projectId: string;
  }>;
}

export interface Report {
  id: string;
  workspaceId: string;
  projectId: string;
  reportType: "project_weekly";
  title: string;
  periodStart: string;
  periodEnd: string;
  status: ReportStatus;
  currentVersion: number;
  content: ProjectWeeklyReportContent;
  generatedAt: string;
  finalizedAt: string | null;
  createdAt: string;
  updatedAt: string;
  version: number;
}

type ListResponse<T> = { items: T[]; nextCursor: string | null };

export async function fetchProjectReports(
  baseUrl: string,
  access: string,
  workspaceId: string,
  projectId: string,
): Promise<Report[]> {
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/reports`,
    browserOrigin(),
  );
  url.searchParams.set("workspaceId", workspaceId);
  url.searchParams.set("projectId", projectId);
  url.searchParams.set("limit", "12");
  const response = await fetch(url, {
    headers: { Accept: "application/json", Authorization: `Bearer ${access}` },
  });
  const body = await readJson(response);
  if (!response.ok || !isListResponse<Report>(body)) {
    throw errorFromStatus(response.status);
  }
  return body.items.filter(isReport);
}

export async function createProjectWeeklyReport(
  baseUrl: string,
  access: string,
  workspaceId: string,
  projectId: string,
): Promise<Report> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/reports/project-weekly`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ workspaceId, projectId }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isReport(body)) throw errorFromStatus(response.status);
  return body;
}

export async function updateReport(
  baseUrl: string,
  access: string,
  report: Report,
  content: ProjectWeeklyReportContent,
): Promise<Report> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/reports/${encodeURIComponent(report.id)}`,
    {
      method: "PUT",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ content, expectedVersion: report.version }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isReport(body)) throw errorFromStatus(response.status);
  return body;
}

export async function finalizeReport(
  baseUrl: string,
  access: string,
  report: Report,
): Promise<Report> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/reports/${encodeURIComponent(report.id)}/finalize`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ expectedVersion: report.version }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isReport(body)) throw errorFromStatus(response.status);
  return body;
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/$/, "");
}

function browserOrigin(): string {
  return typeof window === "undefined"
    ? "http://localhost"
    : window.location.origin;
}

async function readJson(response: Response): Promise<unknown> {
  try {
    return await response.json();
  } catch {
    return null;
  }
}

function errorFromStatus(status: number): PlanningRequestError {
  if (status === 401) return new PlanningRequestError("unauthorized");
  if (status === 409) return new PlanningRequestError("conflict");
  if (status >= 400 && status < 500) return new PlanningRequestError("invalid");
  return new PlanningRequestError("unavailable");
}

function isListResponse<T>(value: unknown): value is ListResponse<T> {
  return (
    typeof value === "object" &&
    value !== null &&
    Array.isArray((value as { items?: unknown }).items)
  );
}

function isReport(value: unknown): value is Report {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<Report>;
  return (
    typeof candidate.id === "string" &&
    typeof candidate.workspaceId === "string" &&
    typeof candidate.projectId === "string" &&
    candidate.reportType === "project_weekly" &&
    typeof candidate.title === "string" &&
    typeof candidate.status === "string" &&
    typeof candidate.version === "number" &&
    isProjectWeeklyReportContent(candidate.content)
  );
}

function isProjectWeeklyReportContent(
  value: unknown,
): value is ProjectWeeklyReportContent {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<ProjectWeeklyReportContent>;
  return (
    candidate.kind === "project_weekly" &&
    typeof candidate.period === "object" &&
    candidate.period !== null &&
    typeof (candidate.period as { start?: unknown }).start === "string" &&
    typeof (candidate.period as { end?: unknown }).end === "string" &&
    typeof candidate.summary === "string" &&
    Array.isArray(candidate.metrics) &&
    candidate.metrics.every(
      (metric) =>
        typeof metric?.key === "string" &&
        typeof metric.label === "string" &&
        (metric.value === null || typeof metric.value === "number"),
    ) &&
    Array.isArray(candidate.focus) &&
    candidate.focus.every((item) => typeof item === "string") &&
    Array.isArray(candidate.evidence) &&
    candidate.evidence.every(
      (item) =>
        typeof item?.type === "string" &&
        typeof item.workspaceId === "string" &&
        typeof item.projectId === "string",
    )
  );
}
