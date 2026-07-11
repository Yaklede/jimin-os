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
    expect(serverBaseUrlFromEnvironment({})).toBeUndefined();
  });
});
