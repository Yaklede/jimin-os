import { PlanningRequestError } from "./planning";

import type { GmailWorkspaceScope } from "./gmail";

export type GmailInflowStatus =
  "pending" | "promoted" | "dismissed" | "deferred";

export type GmailInflowAnalysisStatus =
  "queued" | "claimed" | "running" | "ready" | "failed";

export interface GmailInflowCandidate {
  id: string;
  accountId: string;
  accountEmail: string;
  workspaceId: string;
  workspaceName: string;
  workspaceScope: GmailWorkspaceScope;
  messageId: string;
  providerMessageId: string;
  providerThreadId: string;
  senderName: string | null;
  senderEmail: string;
  subject: string;
  snippet: string;
  bodyText: string | null;
  originalThreadUrl: string | null;
  referenceLinks: string[];
  receivedAt: string;
  suggestedTaskTitle: string;
  suggestedTaskNotes: string;
  suggestedAssigneeName: string | null;
  suggestedPriority: number | null;
  suggestedDueAt: string | null;
  analysisStatus: GmailInflowAnalysisStatus;
  analysisClassification: string | null;
  analysisConfidence: number | null;
  analysisSummary: string | null;
  analysisErrorCode: string | null;
  status: GmailInflowStatus;
  promotedTaskId: string | null;
  deferredUntil: string | null;
  version: number;
}

export interface GmailInflowResponse {
  items: GmailInflowCandidateWire[];
  nextCursor: string | null;
}

export interface GmailInflowFetchResult {
  items: GmailInflowCandidate[];
  nextCursor: string | null;
  partial: boolean;
}

export interface GmailInflowLoadHealth {
  initialFailedWorkspaces: string[];
  loadMoreFailedWorkspaces: string[];
}

export const emptyGmailInflowLoadHealth: GmailInflowLoadHealth = {
  initialFailedWorkspaces: [],
  loadMoreFailedWorkspaces: [],
};

export function gmailInflowHealthAfterInitial(
  failedWorkspaces: string[],
): GmailInflowLoadHealth {
  return {
    initialFailedWorkspaces: uniqueWorkspaceNames(failedWorkspaces),
    loadMoreFailedWorkspaces: [],
  };
}

export function gmailInflowHealthAfterLoadMore(
  current: GmailInflowLoadHealth,
  failedWorkspaces: string[],
): GmailInflowLoadHealth {
  return {
    initialFailedWorkspaces: current.initialFailedWorkspaces,
    loadMoreFailedWorkspaces: uniqueWorkspaceNames(failedWorkspaces),
  };
}

type GmailInflowCandidateWire = Omit<
  GmailInflowCandidate,
  "senderEmail" | "subject" | "snippet" | "receivedAt"
> & {
  senderEmail: string | null;
  subject: string | null;
  snippet: string | null;
  receivedAt: string | null;
};

export type GmailInflowDecision =
  | {
      decision: "promote";
      projectId: string;
      title: string;
      notes: string;
      assigneeName: string | null;
      priority: number;
      dueAt: string | null;
      withoutDeadline: boolean;
    }
  | { decision: "dismiss" }
  | { decision: "defer"; revisitAt: string }
  | { decision: "retry_analysis" };

export async function fetchGmailInflow(
  baseUrl: string,
  access: string,
  workspaceId: string,
  cursor?: string,
): Promise<GmailInflowFetchResult> {
  if (!workspaceId.trim()) throw new PlanningRequestError("invalid");
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/inflow`,
    browserOrigin(),
  );
  url.searchParams.set("workspaceId", workspaceId);
  url.searchParams.set("status", "attention");
  url.searchParams.set("limit", "100");
  if (cursor) url.searchParams.set("cursor", cursor);
  const response = await fetch(url.toString(), {
    headers: requestHeaders(access),
  });
  const body = await readJson(response);
  if (!response.ok || !isGmailInflowResponse(body)) {
    throw errorFromStatus(response.status);
  }
  return {
    items: body.items.map(normalizeGmailInflowCandidate),
    nextCursor: body.nextCursor,
    partial: false,
  };
}

export async function decideGmailInflow(
  baseUrl: string,
  access: string,
  candidate: GmailInflowCandidate,
  decision: GmailInflowDecision,
): Promise<GmailInflowCandidate> {
  validateDecision(candidate, decision);
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/inflow/${encodeURIComponent(candidate.id)}/decision`,
    {
      method: "POST",
      headers: requestHeaders(access, true),
      body: JSON.stringify({
        ...decision,
        expectedVersion: candidate.version,
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGmailInflowCandidate(body)) {
    throw errorFromStatus(response.status);
  }
  return normalizeGmailInflowCandidate(body);
}

function validateDecision(
  candidate: GmailInflowCandidate,
  decision: GmailInflowDecision,
): void {
  if (!candidate.id.trim() || !Number.isSafeInteger(candidate.version)) {
    throw new PlanningRequestError("invalid");
  }
  if (decision.decision === "promote") {
    if (
      !decision.projectId.trim() ||
      !decision.title.trim() ||
      !Number.isSafeInteger(decision.priority) ||
      decision.priority < 0 ||
      decision.priority > 3 ||
      (!decision.withoutDeadline && !decision.dueAt) ||
      (decision.withoutDeadline && decision.dueAt !== null)
    ) {
      throw new PlanningRequestError("invalid");
    }
  }
  if (decision.decision === "defer") {
    const revisitAt = new Date(decision.revisitAt).getTime();
    const now = Date.now();
    if (
      !Number.isFinite(revisitAt) ||
      revisitAt <= now ||
      revisitAt > now + 365 * 24 * 60 * 60 * 1_000
    ) {
      throw new PlanningRequestError("invalid");
    }
  }
}

export function normalizeGmailInflowCandidate(
  candidate: GmailInflowCandidate | GmailInflowCandidateWire,
): GmailInflowCandidate {
  return {
    ...candidate,
    senderName: candidate.senderName?.trim() || null,
    senderEmail: candidate.senderEmail?.trim() || "",
    subject: candidate.subject?.trim() || "",
    snippet: candidate.snippet ?? "",
    bodyText: candidate.bodyText ?? null,
    originalThreadUrl: isTrustedGmailUrl(candidate.originalThreadUrl)
      ? candidate.originalThreadUrl
      : null,
    referenceLinks: Array.isArray(candidate.referenceLinks)
      ? candidate.referenceLinks.filter(isTrustedReferenceUrl)
      : [],
    suggestedTaskTitle:
      candidate.suggestedTaskTitle?.trim() || candidate.subject?.trim() || "",
    suggestedTaskNotes: candidate.suggestedTaskNotes ?? "",
    suggestedAssigneeName: candidate.suggestedAssigneeName ?? null,
    suggestedPriority:
      candidate.suggestedPriority !== null &&
      Number.isSafeInteger(candidate.suggestedPriority) &&
      candidate.suggestedPriority >= 0 &&
      candidate.suggestedPriority <= 3
        ? candidate.suggestedPriority
        : 1,
    suggestedDueAt: normalizeNullableTimestamp(candidate.suggestedDueAt),
    analysisStatus: candidate.analysisStatus ?? "queued",
    analysisClassification: candidate.analysisClassification ?? null,
    analysisConfidence: candidate.analysisConfidence ?? null,
    analysisSummary: candidate.analysisSummary ?? null,
    analysisErrorCode: candidate.analysisErrorCode?.trim() || null,
    status: candidate.status ?? "pending",
    promotedTaskId: candidate.promotedTaskId ?? null,
    deferredUntil: normalizeNullableTimestamp(candidate.deferredUntil),
    receivedAt: normalizeNullableTimestamp(candidate.receivedAt) ?? "",
  };
}

function requestHeaders(access: string, json = false): Record<string, string> {
  return {
    Accept: "application/json",
    Authorization: `Bearer ${access}`,
    ...(json ? { "Content-Type": "application/json" } : {}),
  };
}

function normalizeBaseUrl(value: string): string {
  return value.replace(/\/$/, "");
}

function browserOrigin(): string {
  return typeof window === "undefined"
    ? "https://jimin-os.local"
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
  if (status === 404) return new PlanningRequestError("invalid");
  if (status === 409) return new PlanningRequestError("conflict");
  if (status >= 400 && status < 500) {
    return new PlanningRequestError("invalid");
  }
  return new PlanningRequestError("unavailable");
}

function isGmailInflowResponse(value: unknown): value is GmailInflowResponse {
  return (
    isRecord(value) &&
    Array.isArray(value.items) &&
    value.items.every(isGmailInflowCandidate) &&
    (typeof value.nextCursor === "string" || value.nextCursor === null)
  );
}

function isGmailInflowCandidate(
  value: unknown,
): value is GmailInflowCandidateWire {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.accountId === "string" &&
    typeof value.accountEmail === "string" &&
    typeof value.workspaceId === "string" &&
    typeof value.workspaceName === "string" &&
    isWorkspaceScope(value.workspaceScope) &&
    typeof value.messageId === "string" &&
    typeof value.providerMessageId === "string" &&
    typeof value.providerThreadId === "string" &&
    (typeof value.senderName === "string" || value.senderName === null) &&
    (typeof value.senderEmail === "string" || value.senderEmail === null) &&
    (typeof value.subject === "string" || value.subject === null) &&
    (typeof value.snippet === "string" || value.snippet === null) &&
    (typeof value.bodyText === "string" || value.bodyText === null) &&
    (typeof value.originalThreadUrl === "string" ||
      value.originalThreadUrl === null) &&
    Array.isArray(value.referenceLinks) &&
    value.referenceLinks.every((link) => typeof link === "string") &&
    isNullableTimestamp(value.receivedAt) &&
    typeof value.suggestedTaskTitle === "string" &&
    typeof value.suggestedTaskNotes === "string" &&
    (typeof value.suggestedAssigneeName === "string" ||
      value.suggestedAssigneeName === null) &&
    (value.suggestedPriority === null ||
      (Number.isSafeInteger(value.suggestedPriority) &&
        Number(value.suggestedPriority) >= 0 &&
        Number(value.suggestedPriority) <= 3)) &&
    isNullableTimestamp(value.suggestedDueAt) &&
    isAnalysisStatus(value.analysisStatus) &&
    (typeof value.analysisClassification === "string" ||
      value.analysisClassification === null) &&
    (typeof value.analysisConfidence === "number" ||
      value.analysisConfidence === null) &&
    (value.analysisConfidence === null ||
      (Number.isSafeInteger(value.analysisConfidence) &&
        value.analysisConfidence >= 0 &&
        value.analysisConfidence <= 100)) &&
    (typeof value.analysisSummary === "string" ||
      value.analysisSummary === null) &&
    (typeof value.analysisErrorCode === "string" ||
      value.analysisErrorCode === null) &&
    isInflowStatus(value.status) &&
    (typeof value.promotedTaskId === "string" ||
      value.promotedTaskId === null) &&
    isNullableTimestamp(value.deferredUntil) &&
    Number.isSafeInteger(value.version) &&
    Number(value.version) > 0
  );
}

function isWorkspaceScope(value: unknown): value is GmailWorkspaceScope {
  return value === "personal" || value === "company";
}

function isAnalysisStatus(value: unknown): value is GmailInflowAnalysisStatus {
  return (
    value === "queued" ||
    value === "claimed" ||
    value === "running" ||
    value === "ready" ||
    value === "failed"
  );
}

function isInflowStatus(value: unknown): value is GmailInflowStatus {
  return (
    value === "pending" ||
    value === "promoted" ||
    value === "dismissed" ||
    value === "deferred"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isNullableTimestamp(value: unknown): value is string | null {
  return (
    value === null ||
    (typeof value === "string" &&
      value.trim().length > 0 &&
      Number.isFinite(Date.parse(value)))
  );
}

function normalizeNullableTimestamp(value: string | null): string | null {
  return value && Number.isFinite(Date.parse(value)) ? value : null;
}

function uniqueWorkspaceNames(values: string[]): string[] {
  return [...new Set(values.map((value) => value.trim()).filter(Boolean))];
}

export function isTrustedGmailUrl(value: string | null): value is string {
  if (!value) return false;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "mail.google.com";
  } catch {
    return false;
  }
}

function isTrustedReferenceUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "https:" || url.protocol === "http:";
  } catch {
    return false;
  }
}
