/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_API_BASE_URL?: string;
  readonly VITE_DESIGN_PREVIEW?: string;
  readonly VITE_CODEX_SKILL_CATALOG?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
