import { describe, expect, it, vi } from "vitest";

import { retryUnauthorizedRequest } from "./session-retry";

function session(access: string, refresh: string) {
  return {
    ["accessToken"]: access,
    ["refreshToken"]: refresh,
  };
}

describe("session retry", () => {
  it("refreshes and replays a request rejected by an expired access token", async () => {
    const execute = vi
      .fn<(access: string) => Promise<string>>()
      .mockRejectedValueOnce({ code: "unauthorized" })
      .mockResolvedValueOnce("처리했어요");
    const refresh = vi.fn().mockResolvedValue(session("renewed", "rotated"));

    await expect(
      retryUnauthorizedRequest(session("expired", "current"), execute, refresh),
    ).resolves.toBe("처리했어요");

    expect(refresh).toHaveBeenCalledWith("current");
    expect(execute).toHaveBeenNthCalledWith(1, "expired");
    expect(execute).toHaveBeenNthCalledWith(2, "renewed");
  });

  it("does not rotate the session for a request error that is not authorization", async () => {
    const execute = vi
      .fn<(access: string) => Promise<string>>()
      .mockRejectedValue(new Error("network unavailable"));
    const refresh = vi.fn();

    await expect(
      retryUnauthorizedRequest(session("access", "refresh"), execute, refresh),
    ).rejects.toThrow("network unavailable");

    expect(refresh).not.toHaveBeenCalled();
  });
});
