import type { SessionTokens } from "./api/planning";

type UnauthorizedFailure = {
  code?: unknown;
};

/**
 * Replays a single user action after the private-server session rotates.
 *
 * The caller owns refresh de-duplication because several screens can make a
 * request at the same time after an access token expires.
 */
export async function retryUnauthorizedRequest<T>(
  session: SessionTokens,
  execute: (access: string) => Promise<T>,
  refresh: (refreshValue: string) => Promise<SessionTokens>,
): Promise<T> {
  try {
    return await execute(session.accessToken);
  } catch (error) {
    if (!isUnauthorizedFailure(error)) throw error;

    const refreshed = await refresh(session.refreshToken);
    return execute(refreshed.accessToken);
  }
}

export function isUnauthorizedFailure(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    (error as UnauthorizedFailure).code === "unauthorized"
  );
}
