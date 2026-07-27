import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(import.meta.url);

export function mobileMcpEntry() {
  return require.resolve("@mobilenext/mobile-mcp/lib/index.js");
}

export function mobileCliBinary(
  platform = process.platform,
  architecture = process.arch,
) {
  const normalizedPlatform = platform === "win32" ? "windows" : platform;
  const normalizedArchitecture = architecture === "arm64" ? "arm64" : "amd64";
  const extension = platform === "win32" ? ".exe" : "";
  const packageDirectory = path.dirname(
    require.resolve("mobilecli/package.json"),
  );
  return path.join(
    packageDirectory,
    "bin",
    `mobilecli-${normalizedPlatform}-${normalizedArchitecture}${extension}`,
  );
}

export function mobileMcpEnvironment(overrides = {}) {
  return {
    ...process.env,
    MOBILECLI_PATH: mobileCliBinary(),
    MOBILEMCP_DISABLE_TELEMETRY: "1",
    ...overrides,
  };
}
