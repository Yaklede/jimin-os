import { describe, expect, it, vi } from "vitest";

import { HealthRequestError, fetchServerHealth } from "./health";

function response(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

describe("fetchServerHealth", () => {
  it("returns the live and ready snapshots", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        response({ status: "ok", service: "api", buildSha: "abc123" }),
      )
      .mockResolvedValueOnce(
        response({
          status: "ready",
          checks: { configuration: "ok", database: "ok", migrations: "ok" },
          schemaVersion: 1,
        }),
      );

    const snapshot = await fetchServerHealth("/server/", fetchMock);

    expect(snapshot.live.buildSha).toBe("abc123");
    expect(snapshot.ready.status).toBe("ready");
    expect(fetchMock).toHaveBeenNthCalledWith(
      1,
      "/server/health/live",
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it("keeps a 503 readiness response as a recoverable state", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(
        response({ status: "ok", service: "api", buildSha: "abc123" }),
      )
      .mockResolvedValueOnce(
        response(
          {
            status: "notReady",
            checks: {
              configuration: "ok",
              database: "error",
              migrations: "error",
            },
            schemaVersion: 1,
          },
          503,
        ),
      );

    const snapshot = await fetchServerHealth("/server", fetchMock);

    expect(snapshot.ready.status).toBe("notReady");
    expect(snapshot.ready.checks.database).toBe("error");
  });

  it("rejects malformed or unreachable responses without exposing details", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockRejectedValue(new Error("connection refused"));

    await expect(
      fetchServerHealth("/server", fetchMock),
    ).rejects.toBeInstanceOf(HealthRequestError);
  });
});
