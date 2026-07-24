import { describe, expect, it, vi } from "vitest";

import { handleMobileBack, registerMobileBackHandler } from "./mobileBack";

describe("mobile back handlers", () => {
  it("uses the highest-priority handler that consumes the action", () => {
    const route = vi.fn(() => true);
    const sheet = vi.fn(() => true);
    const unregisterRoute = registerMobileBackHandler(route, 10);
    const unregisterSheet = registerMobileBackHandler(sheet, 100);

    expect(handleMobileBack()).toBe(true);
    expect(sheet).toHaveBeenCalledOnce();
    expect(route).not.toHaveBeenCalled();

    unregisterSheet();
    unregisterRoute();
  });

  it("falls through handlers that do not consume the action", () => {
    const route = vi.fn(() => true);
    const inactiveOverlay = vi.fn(() => false);
    const unregisterRoute = registerMobileBackHandler(route, 10);
    const unregisterOverlay = registerMobileBackHandler(inactiveOverlay, 100);

    expect(handleMobileBack()).toBe(true);
    expect(inactiveOverlay).toHaveBeenCalledOnce();
    expect(route).toHaveBeenCalledOnce();

    unregisterOverlay();
    unregisterRoute();
  });

  it("returns false at the app root", () => {
    const inactive = vi.fn(() => false);
    const unregister = registerMobileBackHandler(inactive);

    expect(handleMobileBack()).toBe(false);

    unregister();
  });
});
