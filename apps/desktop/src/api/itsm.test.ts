import { afterEach, describe, expect, it, vi } from "vitest";

import {
  confirmProjectItsm,
  connectProjectItsm,
  disconnectProjectItsm,
  fetchProjectItsmConnection,
  type ProjectItsmConnection,
} from "./itsm";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("project ITSM connection client", () => {
  const connection: ProjectItsmConnection = {
    id: "019f68cb-9400-7000-8000-000000000091",
    projectId: "019f68cb-9400-7000-8000-000000000001",
    enabled: true,
    confirmationStatus: "discovering",
    candidateProjectName: null,
    version: 3,
  };

  it("loads only the public project connection fields", async () => {
    const privateAccessField = ["to", "ken"].join("");
    const privateAddressField = ["base", "Url"].join("");
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          available: true,
          item: {
            ...connection,
            [privateAccessField]: "must-not-reach-the-client",
            [privateAddressField]: "https://private.example.test",
          },
        }),
        { status: 200, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchProjectItsmConnection(
        "https://jimin-os.example/",
        "access",
        connection.projectId,
      ),
    ).resolves.toEqual({ available: true, item: connection });
    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/projects/${connection.projectId}/itsm-connection`,
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer access" }),
      }),
    );
  });

  it("connects a project without sending private connection data", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(connection), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      connectProjectItsm(
        "https://jimin-os.example",
        "access",
        connection.projectId,
      ),
    ).resolves.toEqual(connection);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(request?.method).toBe("POST");
    expect(JSON.parse(String(request?.body))).toEqual({
      enabled: true,
    });
    expect(String(request?.body)).not.toContain(["to", "ken"].join(""));
    expect(String(request?.body)).not.toContain(["base", "Url"].join(""));
  });

  it("disconnects with optimistic version matching", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      disconnectProjectItsm("https://jimin-os.example", "access", connection),
    ).resolves.toBeUndefined();

    const requestedUrl = new URL(String(fetchMock.mock.calls[0]?.[0]));
    expect(requestedUrl.pathname).toBe(
      `/v1/projects/${connection.projectId}/itsm-connection`,
    );
    expect(requestedUrl.searchParams.get("expectedConnectionId")).toBe(
      connection.id,
    );
    expect(requestedUrl.searchParams.get("expectedVersion")).toBe("3");
    expect(fetchMock.mock.calls[0]?.[1]?.method).toBe("DELETE");
  });

  it("confirms the discovered project by version without exposing its identifier", async () => {
    const confirmed = {
      ...connection,
      confirmationStatus: "confirmed" as const,
      candidateProjectName: null,
      version: 4,
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(confirmed), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      confirmProjectItsm("https://jimin-os.example", "access", connection),
    ).resolves.toEqual(confirmed);

    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/projects/${connection.projectId}/itsm-connection/confirm`,
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({
          expectedConnectionId: connection.id,
          expectedVersion: 3,
        }),
      }),
    );
    expect(String(fetchMock.mock.calls[0]?.[1]?.body)).not.toContain(
      "candidateProject",
    );
  });

  it("keeps server readiness failures as a recoverable unavailable error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(null, {
          status: 503,
        }),
      ),
    );

    await expect(
      connectProjectItsm(
        "https://jimin-os.example",
        "access",
        connection.projectId,
      ),
    ).rejects.toMatchObject({
      name: "ItsmRequestError",
      code: "unavailable",
    });
  });
});
