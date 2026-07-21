import { afterEach, describe, expect, it, vi } from "vitest";

import {
  disablePushRegistration,
  fetchPushRegistration,
  registerFcmToken,
} from "./push";

const registration = {
  enabled: true,
  provider: "fcm",
  lastSeenAt: "2026-07-21T01:00:00Z",
  lastDeliveredAt: null,
  lastErrorCode: null,
};
const registrationHandle = "private-fcm-registration-handle";
const fcmRegistrationField = "token";

describe("push registration API", () => {
  afterEach(() => vi.restoreAllMocks());

  it("registers a handle only in the authenticated request body", async () => {
    const request = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(JSON.stringify(registration)));
    await expect(
      registerFcmToken(
        "https://os.jimin.ai.kr/",
        "session-access",
        registrationHandle,
      ),
    ).resolves.toEqual(registration);
    expect(request).toHaveBeenCalledWith(
      "https://os.jimin.ai.kr/v1/push/registration",
      expect.objectContaining({
        method: "PUT",
        headers: expect.objectContaining({
          Authorization: "Bearer session-access",
        }),
        body: JSON.stringify({
          provider: "fcm",
          [fcmRegistrationField]: registrationHandle,
        }),
      }),
    );
  });

  it("reads safe metadata without expecting a registration credential", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify(registration)),
    );
    await expect(
      fetchPushRegistration("https://os.jimin.ai.kr", "access"),
    ).resolves.toEqual(registration);
    expect(JSON.stringify(registration)).not.toContain(fcmRegistrationField);
  });

  it("disables the current device registration", async () => {
    const request = vi
      .spyOn(globalThis, "fetch")
      .mockResolvedValue(new Response(null, { status: 204 }));
    await expect(
      disablePushRegistration("https://os.jimin.ai.kr", "access"),
    ).resolves.toBeUndefined();
    expect(request).toHaveBeenCalledWith(
      "https://os.jimin.ai.kr/v1/push/registration",
      expect.objectContaining({ method: "DELETE" }),
    );
  });
});
