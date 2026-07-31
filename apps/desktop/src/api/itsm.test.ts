import { afterEach, describe, expect, it, vi } from "vitest";

import {
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
    itsmProjectId: "42",
    enabled: true,
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
        connection.itsmProjectId,
      ),
    ).resolves.toEqual(connection);

    const request = fetchMock.mock.calls[0]?.[1];
    expect(request?.method).toBe("POST");
    expect(JSON.parse(String(request?.body))).toEqual({
      enabled: true,
      itsmProjectId: "42",
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
    expect(requestedUrl.searchParams.get("expectedVersion")).toBe("3");
    expect(fetchMock.mock.calls[0]?.[1]?.method).toBe("DELETE");
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
        connection.itsmProjectId,
      ),
    ).rejects.toMatchObject({
      name: "ItsmRequestError",
      code: "unavailable",
    });
  });
});
