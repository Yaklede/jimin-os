import { afterEach, describe, expect, it, vi } from "vitest";
import { fetchOpenTasks, PlanningRequestError } from "./planning";

afterEach(() => vi.unstubAllGlobals());
describe("mobile complete task listing", () => {
  it("requests the guarded all-open endpoint without an unsupported page limit", async () => {
    const mock = vi
      .fn()
      .mockResolvedValue(
        new Response(
          JSON.stringify({
            items: Array.from({ length: 177 }, (_, i) => ({ id: String(i) })),
            nextCursor: null,
          }),
        ),
      );
    vi.stubGlobal("fetch", mock);
    expect(await fetchOpenTasks("https://os.example", "token")).toHaveLength(
      177,
    );
    expect(mock).toHaveBeenCalledWith(
      "https://os.example/v1/tasks?status=open",
      {
        headers: { Accept: "application/json", Authorization: "Bearer token" },
      },
    );
  });
  it("does not silently call a partial page the entire list", async () => {
    vi.stubGlobal(
      "fetch",
      vi
        .fn()
        .mockResolvedValue(
          new Response(JSON.stringify({ items: [], nextCursor: "next" })),
        ),
    );
    await expect(
      fetchOpenTasks("https://os.example", "token"),
    ).rejects.toBeInstanceOf(PlanningRequestError);
  });
  it("preserves unauthorized failures for the existing session refresh", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(new Response("{}", { status: 401 })),
    );
    await expect(
      fetchOpenTasks("https://os.example", "token"),
    ).rejects.toMatchObject({ code: "unauthorized" });
  });
});
