import { PlanningRequestError, clientPlatformForUserAgent } from "./planning";

export type GmailWorkspaceScope = "personal" | "company";

export type GmailAccountStatus =
  | "connecting"
  | "active"
  | "reauth_required"
  | "revoking"
  | "revoked"
  | "error";

export interface GmailAccount {
  id: string;
  workspaceId: string;
  workspaceScope: GmailWorkspaceScope;
  workspaceName: string;
  email: string;
  status: GmailAccountStatus;
  lastSuccessfulSyncAt: string | null;
  lastErrorCode: string | null;
  reauthRequired: boolean;
  version: number;
}

export interface GmailAccountsResponse {
  available: boolean;
  items: GmailAccount[];
}

export interface GmailAuthorization {
  authorizationId: string;
  authorizationUrl: string;
  expiresAt: string;
}

export type GmailAuthorizationAccountSnapshot = Record<
  string,
  Pick<GmailAccount, "status" | "version">
>;

export interface GmailAuthorizationBaseline {
  workspaceId: string;
  accountId?: string;
  accounts: GmailAuthorizationAccountSnapshot;
}

export async function fetchGmailAccounts(
  baseUrl: string,
  access: string,
): Promise<GmailAccountsResponse> {
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/accounts`,
    {
      headers: requestHeaders(access),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGmailAccountsResponse(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function startGmailAuthorization(
  baseUrl: string,
  access: string,
  workspaceId: string,
  options: { accountId?: string; userAgent?: string } = {},
): Promise<GmailAuthorization> {
  if (!workspaceId.trim()) {
    throw new PlanningRequestError("invalid");
  }
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/accounts/authorizations`,
    {
      method: "POST",
      headers: requestHeaders(access, true),
      body: JSON.stringify({
        clientKind: clientPlatformForUserAgent(
          options.userAgent ??
            (typeof navigator === "undefined" ? "" : navigator.userAgent),
        ),
        workspaceId,
        ...(options.accountId ? { accountId: options.accountId } : {}),
      }),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGmailAuthorization(body)) {
    throw errorFromStatus(response.status);
  }
  if (!isTrustedGoogleAuthorizationUrl(body.authorizationUrl)) {
    throw new PlanningRequestError("unavailable");
  }
  return body;
}

export async function synchronizeGmailAccount(
  baseUrl: string,
  access: string,
  accountId: string,
): Promise<GmailAccount> {
  if (!accountId.trim()) {
    throw new PlanningRequestError("invalid");
  }
  const response = await fetch(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/accounts/${encodeURIComponent(accountId)}/sync`,
    {
      method: "POST",
      headers: requestHeaders(access),
    },
  );
  const body = await readJson(response);
  if (!response.ok || !isGmailAccount(body)) {
    throw errorFromStatus(response.status);
  }
  return body;
}

export async function disconnectGmailAccount(
  baseUrl: string,
  access: string,
  accountId: string,
  expectedVersion: number,
): Promise<void> {
  if (
    !accountId.trim() ||
    !Number.isSafeInteger(expectedVersion) ||
    expectedVersion <= 0
  ) {
    throw new PlanningRequestError("invalid");
  }
  const url = new URL(
    `${normalizeBaseUrl(baseUrl)}/v1/gmail/accounts/${encodeURIComponent(accountId)}`,
  );
  url.searchParams.set("expectedVersion", String(expectedVersion));
  const response = await fetch(url.toString(), {
    method: "DELETE",
    headers: requestHeaders(access),
  });
  if (!response.ok) {
    throw errorFromStatus(response.status);
  }
}

export function gmailAuthorizationBaseline(
  workspaceId: string,
  accounts: GmailAccount[],
  accountId?: string,
): GmailAuthorizationBaseline {
  return {
    workspaceId,
    accountId,
    accounts: Object.fromEntries(
      accounts
        .filter((account) => account.workspaceId === workspaceId)
        .map((account) => [
          account.id,
          { status: account.status, version: account.version },
        ]),
    ),
  };
}

export function gmailAuthorizationChanged(
  baseline: GmailAuthorizationBaseline,
  accounts: GmailAccount[],
): boolean {
  const workspaceAccounts = accounts.filter(
    (account) => account.workspaceId === baseline.workspaceId,
  );
  if (baseline.accountId) {
    const previous = baseline.accounts[baseline.accountId];
    const current = workspaceAccounts.find(
      (account) => account.id === baseline.accountId,
    );
    return Boolean(
      previous &&
      current &&
      (current.version !== previous.version ||
        current.status !== previous.status),
    );
  }
  return workspaceAccounts.some((account) => !baseline.accounts[account.id]);
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
  if (status >= 400 && status < 500) return new PlanningRequestError("invalid");
  return new PlanningRequestError("unavailable");
}

function isGmailAccountsResponse(
  value: unknown,
): value is GmailAccountsResponse {
  return (
    isRecord(value) &&
    typeof value.available === "boolean" &&
    Array.isArray(value.items) &&
    value.items.every(isGmailAccount)
  );
}

function isGmailAccount(value: unknown): value is GmailAccount {
  return (
    isRecord(value) &&
    typeof value.id === "string" &&
    typeof value.workspaceId === "string" &&
    isWorkspaceScope(value.workspaceScope) &&
    typeof value.workspaceName === "string" &&
    typeof value.email === "string" &&
    isAccountStatus(value.status) &&
    (typeof value.lastSuccessfulSyncAt === "string" ||
      value.lastSuccessfulSyncAt === null) &&
    (typeof value.lastErrorCode === "string" || value.lastErrorCode === null) &&
    typeof value.reauthRequired === "boolean" &&
    typeof value.version === "number" &&
    Number.isSafeInteger(value.version) &&
    value.version > 0
  );
}

function isGmailAuthorization(value: unknown): value is GmailAuthorization {
  return (
    isRecord(value) &&
    typeof value.authorizationId === "string" &&
    typeof value.authorizationUrl === "string" &&
    typeof value.expiresAt === "string"
  );
}

function isTrustedGoogleAuthorizationUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "https:" && url.hostname === "accounts.google.com";
  } catch {
    return false;
  }
}

function isWorkspaceScope(value: unknown): value is GmailWorkspaceScope {
  return value === "personal" || value === "company";
}

function isAccountStatus(value: unknown): value is GmailAccountStatus {
  return (
    typeof value === "string" &&
    [
      "connecting",
      "active",
      "reauth_required",
      "revoking",
      "revoked",
      "error",
    ].includes(value)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
