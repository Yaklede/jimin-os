import { describe, expect, it } from "vitest";

import { shouldUseNativeSecureStore } from "./device-session";

describe("shouldUseNativeSecureStore", () => {
  it("keeps production Tauri sessions in the native secure store", () => {
    expect(shouldUseNativeSecureStore(true, undefined)).toBe(true);
  });

  it("uses browser storage for explicitly marked local test builds", () => {
    expect(shouldUseNativeSecureStore(true, "1")).toBe(false);
  });

  it("uses browser storage outside the Tauri runtime", () => {
    expect(shouldUseNativeSecureStore(false, undefined)).toBe(false);
  });
});
