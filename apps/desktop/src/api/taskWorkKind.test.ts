import { afterEach, describe, expect, it, vi } from "vitest";
import { createTask, updateTask, completeTask, type Task } from "./planning";
afterEach(() => vi.unstubAllGlobals());
describe("task type and result transport", () => {
  it("sends the chosen type on creation/update and a trimmed result on completion", async () => {
    const task = {
      id: "task",
      projectId: null,
      version: 3,
      workKind: "verification",
    } as Task;
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockImplementation(async () => Response.json(task));
    vi.stubGlobal("fetch", fetchMock);
    await createTask("https://example.test", "test", {
      title: "금액 확인",
      priority: 1,
      workKind: "verification",
    });
    await updateTask("https://example.test", "test", task, {
      title: "금액 개발",
      priority: 1,
      status: "open",
      workKind: "development",
    });
    await completeTask(
      "https://example.test",
      "test",
      task,
      "  금액이 모두 일치해요.  ",
    );
    const payloads = fetchMock.mock.calls.map(([, init]) =>
      JSON.parse(String(init?.body)),
    );
    expect(payloads[0].workKind).toBe("verification");
    expect(payloads[1].workKind).toBe("development");
    expect(payloads[2]).toEqual({
      expectedVersion: 3,
      completionNote: "금액이 모두 일치해요.",
    });
  });
  it("preserves the existing type when older clients omit it and permits completion without a note", async () => {
    const task = { id: "task", projectId: null, version: 1 } as Task;
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockImplementation(async () => Response.json(task));
    vi.stubGlobal("fetch", fetchMock);
    await updateTask("https://example.test", "test", task, {
      title: "기존 업무",
      priority: 1,
      status: "open",
    });
    await completeTask("https://example.test", "test", task);
    expect(
      JSON.parse(String(fetchMock.mock.calls[0][1]?.body)),
    ).not.toHaveProperty("workKind");
    expect(JSON.parse(String(fetchMock.mock.calls[1][1]?.body))).toEqual({
      expectedVersion: 1,
    });
  });
});
