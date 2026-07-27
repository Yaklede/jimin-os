import { spawn } from "node:child_process";

import { mobileMcpEntry, mobileMcpEnvironment } from "./mobile-mcp-runtime.mjs";

const child = spawn(process.execPath, [mobileMcpEntry(), "--stdio"], {
  env: mobileMcpEnvironment(),
  stdio: "inherit",
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}

child.once("error", (error) => {
  console.error(`Mobile MCP를 시작하지 못했습니다: ${error.message}`);
  process.exitCode = 1;
});

child.once("exit", (code, signal) => {
  if (signal) {
    process.exitCode = 128;
    return;
  }
  process.exitCode = code ?? 1;
});
