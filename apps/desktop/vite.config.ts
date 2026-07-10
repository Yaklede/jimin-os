import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

const serverTarget =
  process.env.JIMIN_API_DEV_TARGET ?? "https://localhost:8443";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 1420,
    strictPort: true,
    proxy: {
      "/server": {
        target: serverTarget,
        changeOrigin: true,
        secure: false,
        rewrite: (path) => path.replace(/^\/server/, ""),
      },
    },
  },
  preview: {
    port: 1421,
    strictPort: true,
  },
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },
});
