import { useEffect, useState } from "react";
import { mobileCapabilitySnapshot } from "./mobile-capabilities";

export function useMobileLayout() {
  const [mobile, setMobile] = useState(
    () =>
      typeof window !== "undefined" &&
      mobileCapabilitySnapshot().platform !== "desktop" &&
      window.matchMedia("(max-width: 720px)").matches,
  );
  useEffect(() => {
    const media = window.matchMedia("(max-width: 720px)");
    const update = () =>
      setMobile(
        mobileCapabilitySnapshot().platform !== "desktop" && media.matches,
      );
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  return mobile;
}

/** Keep the keyboard from covering actions without moving the page on tab changes. */
export function useMobileViewport() {
  useEffect(() => {
    if (mobileCapabilitySnapshot().platform === "desktop") return;
    const root = document.documentElement;
    const viewport = window.visualViewport;
    const media = window.matchMedia("(max-width: 720px)");
    let fullHeight = window.innerHeight;
    let previousWidth = window.innerWidth;
    const update = () => {
      const editing = document.activeElement?.matches(
        "input, textarea, select",
      );
      const height = viewport?.height ?? window.innerHeight;
      if (Math.abs(window.innerWidth - previousWidth) > 80)
        fullHeight = window.innerHeight;
      else if (!editing) fullHeight = Math.max(fullHeight, window.innerHeight);
      previousWidth = window.innerWidth;
      const keyboard =
        media.matches &&
        editing &&
        Math.max(fullHeight, window.innerHeight) - height > 140;
      root.dataset.mobileKeyboard = keyboard ? "open" : "closed";
      root.dataset.mobileLayout = media.matches ? "true" : "false";
      root.style.setProperty(
        "--mobile-viewport-height",
        `${viewport?.height ?? window.innerHeight}px`,
      );
    };
    update();
    viewport?.addEventListener("resize", update);
    window.addEventListener("resize", update);
    document.addEventListener("focusin", update);
    document.addEventListener("focusout", update);
    return () => {
      viewport?.removeEventListener("resize", update);
      window.removeEventListener("resize", update);
      document.removeEventListener("focusin", update);
      document.removeEventListener("focusout", update);
      delete root.dataset.mobileKeyboard;
      delete root.dataset.mobileLayout;
      root.style.removeProperty("--mobile-viewport-height");
    };
  }, []);
}
