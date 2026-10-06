import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import App from "./App";
import { initializeAppearance } from "./appearance-preferences";
import "./styles.css";

const root = document.getElementById("root");

if (!root) {
  document.body.insertAdjacentHTML(
    "beforeend",
    '<p role="alert">화면을 불러오지 못했어요. 새로고침해 주세요.</p>',
  );
  throw new Error("App root is missing");
}

const designPreview =
  import.meta.env.VITE_DESIGN_PREVIEW === "1" ||
  (import.meta.env.DEV &&
    new URLSearchParams(location.search).get("preview") === "1");
initializeAppearance(designPreview);

if (designPreview) {
  const { installDesignPreview } = await import("./design-preview");
  installDesignPreview();
  if (import.meta.env.VITE_DESIGN_PREVIEW === "1") {
    const { saveDeviceSession } = await import("./device-session");
    await saveDeviceSession({
      tokens: {
        accessToken: "design-preview",
        refreshToken: "design-preview",
      },
    });
  }
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
