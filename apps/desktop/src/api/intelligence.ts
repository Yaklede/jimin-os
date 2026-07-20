import { createUuidV7 } from "../uuid";
import { type Recommendation } from "./home";
import { PlanningRequestError } from "./planning";

export type RecommendationDecision = "approve" | "reject" | "defer";

interface RecommendationListResponse {
  items: Recommendation[];
}

export async function refreshWorkBrief(
  baseUrl: string,
  access: string,
): Promise<Recommendation[]> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/briefs/work/refresh`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
      },
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isRecommendationListResponse(body)) {
    throw errorFromStatus(response.status);
  }
  return body.items;
}

export async function fetchRecommendationHistory(
  baseUrl: string,
  access: string,
  limit = 50,
): Promise<Recommendation[]> {
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/recommendations`,
    browserOrigin(),
  );
  url.searchParams.set("scope", "all");
  url.searchParams.set("limit", String(limit));
  const response = await fetch(url.toString(), {
    headers: {
      Accept: "application/json",
      Authorization: `Bearer ${access}`,
    },
  });
  const body = await readJson(response);
  if (!response.ok || !isRecommendationListResponse(body)) {
    throw errorFromStatus(response.status);
  }
  return body.items;
}

export async function decideRecommendation(
  baseUrl: string,
  access: string,
  recommendation: Recommendation,
  decision: RecommendationDecision,
  revisitAt?: string,
): Promise<Recommendation> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/recommendations/${recommendation.id}/decisions`,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/json",
        Authorization: `Bearer ${access}`,
      },
      body: JSON.stringify({
        clientMutationId: createUuidV7(),
        decision,
        reason: null,
        revisitAt: revisitAt ?? null,
        expectedVersion: recommendation.version,
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isRecommendation(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
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
  if (status === 409) return new PlanningRequestError("conflict");
  if (status >= 400 && status < 500) return new PlanningRequestError("invalid");
  return new PlanningRequestError("unavailable");
}

function isRecommendationListResponse(
  value: unknown,
): value is RecommendationListResponse {
  return isRecord(value) && Array.isArray(value.items);
}

function isRecommendation(value: unknown): value is Recommendation {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.title === "string" &&
    typeof value.version === "number"
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
