import { describe, expect, it } from "vitest";

import { serverBaseUrlFromEnvironment } from "./server-config";

describe("serverBaseUrlFromEnvironment", () => {
  it("uses the fixed HTTPS server configured at build time", () => {
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "https://jimin-os.example/",
      }),
    ).toBe("https://jimin-os.example");
  });

  it("uses the Vite proxy only during browser development", () => {
    expect(serverBaseUrlFromEnvironment({ DEV: true })).toBe("/server");
  });

  it("uses the private personal server in packaged apps", () => {
    expect(serverBaseUrlFromEnvironment({})).toBe("https://os.jimin.ai.kr");
  });

  it("does not accept an insecure or malformed production origin", () => {
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "http://jimin-os.example",
      }),
    ).toBeUndefined();
    const credentialedUrl = new URL("https://jimin-os.example");
    credentialedUrl.username = "user";
    credentialedUrl.password = "pw";
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: credentialedUrl.toString(),
      }),
    ).toBeUndefined();
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "https://jimin-os.example/api",
      }),
    ).toBeUndefined();
  });

  it("allows HTTP only for the explicit USB loopback test build", () => {
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "http://127.0.0.1:8080",
        VITE_LOCAL_PHONE_TEST: "1",
      }),
    ).toBe("http://127.0.0.1:8080");
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "http://127.0.0.1:8080",
      }),
    ).toBeUndefined();
    expect(
      serverBaseUrlFromEnvironment({
        VITE_API_BASE_URL: "http://192.168.0.195:8080",
        VITE_LOCAL_PHONE_TEST: "1",
      }),
    ).toBeUndefined();
  });
});
