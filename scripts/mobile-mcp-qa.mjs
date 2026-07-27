import { mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

import { mobileMcpEntry, mobileMcpEnvironment } from "./mobile-mcp-runtime.mjs";

const defaultPackageName = "io.jimin.os.dev";

export function parseToolJson(result) {
  if (result?.isError) {
    throw new Error(toolText(result) || "Mobile MCP 명령이 실패했습니다.");
  }
  const text = toolText(result);
  if (!text) return {};
  try {
    return JSON.parse(text);
  } catch {
    throw new Error(`Mobile MCP가 올바른 JSON을 반환하지 않았습니다: ${text}`);
  }
}

export function toolValue(result) {
  if (result?.isError) {
    throw new Error(toolText(result) || "Mobile MCP 명령이 실패했습니다.");
  }
  const text = toolText(result);
  if (!text) return "";
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export function devicesFrom(result) {
  const parsed = parseToolJson(result);
  const devices = Array.isArray(parsed.devices)
    ? parsed.devices
    : Array.isArray(parsed.data?.devices)
      ? parsed.data.devices
      : [];
  return devices.filter(
    (device) => device && typeof device === "object" && deviceId(device),
  );
}

export function deviceId(device) {
  for (const key of ["id", "deviceId", "identifier", "serial"]) {
    if (typeof device?.[key] === "string" && device[key].trim()) {
      return device[key].trim();
    }
  }
  return undefined;
}

export function selectSafeDevice(devices, requestedId) {
  const selected = requestedId
    ? devices.find((device) => deviceId(device) === requestedId)
    : devices.find(isVirtualDevice);
  if (!selected) {
    if (!requestedId && devices.length > 0) {
      throw new Error(
        "운영 실기기는 자동 QA 대상에서 제외했습니다. 에뮬레이터나 시뮬레이터를 실행해 주세요.",
      );
    }
    throw new Error(
      requestedId
        ? `요청한 기기를 찾지 못했습니다: ${requestedId}`
        : "실행 중인 Android 에뮬레이터나 iOS 시뮬레이터가 없습니다.",
    );
  }
  if (!isVirtualDevice(selected)) {
    throw new Error(
      "기본 QA는 운영 실기기를 변경하지 않습니다. 에뮬레이터나 시뮬레이터를 사용해 주세요.",
    );
  }
  return selected;
}

export function parseArguments(values) {
  const [command = "doctor", ...rest] = values;
  const options = {
    command,
    device: undefined,
    packageName: defaultPackageName,
    outputDirectory: path.resolve(".mobile-mcp/artifacts"),
  };
  for (const argument of rest) {
    if (argument === "--") {
      continue;
    } else if (argument.startsWith("--device=")) {
      options.device = argument.slice("--device=".length);
    } else if (argument.startsWith("--package=")) {
      options.packageName = argument.slice("--package=".length);
    } else if (argument.startsWith("--output=")) {
      options.outputDirectory = path.resolve(
        argument.slice("--output=".length),
      );
    } else {
      throw new Error(`지원하지 않는 옵션입니다: ${argument}`);
    }
  }
  if (!["doctor", "smoke"].includes(command)) {
    throw new Error(`지원하지 않는 Mobile QA 명령입니다: ${command}`);
  }
  if (!/^[a-zA-Z][a-zA-Z0-9_.]+$/.test(options.packageName)) {
    throw new Error("Android package 이름을 확인해 주세요.");
  }
  if (options.packageName === "io.jimin.os") {
    throw new Error(
      "운영 패키지는 자동 QA 대상에서 제외했습니다. io.jimin.os.dev를 사용해 주세요.",
    );
  }
  return options;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [mobileMcpEntry(), "--stdio"],
    env: mobileMcpEnvironment(),
    stderr: "pipe",
  });
  const client = new Client({
    name: "jimin-os-mobile-qa",
    version: "0.1.0",
  });

  await client.connect(transport);
  try {
    const { tools } = await client.listTools();
    const missing = requiredTools.filter(
      (name) => !tools.some((tool) => tool.name === name),
    );
    if (missing.length > 0) {
      throw new Error(`필수 Mobile MCP 도구가 없습니다: ${missing.join(", ")}`);
    }

    const deviceResult = await call(
      client,
      "mobile_list_available_devices",
      {},
    );
    const devices = devicesFrom(deviceResult);
    if (options.command === "doctor") {
      console.log(
        JSON.stringify(
          {
            status: "ready",
            telemetry: "disabled",
            toolCount: tools.length,
            devices: devices.map(deviceSummary),
          },
          null,
          2,
        ),
      );
      return;
    }

    const device = selectSafeDevice(devices, options.device);
    const id = deviceId(device);
    const apps = toolValue(
      await call(client, "mobile_list_apps", { device: id }),
    );
    if (!JSON.stringify(apps).includes(options.packageName)) {
      throw new Error(
        `${options.packageName} 앱이 ${id}에 설치되어 있지 않습니다.`,
      );
    }

    await call(client, "mobile_terminate_app", {
      device: id,
      packageName: options.packageName,
    });
    await call(client, "mobile_launch_app", {
      device: id,
      packageName: options.packageName,
      locale: "ko-KR",
    });
    await delay(1_000);

    const firstElements = toolValue(
      await call(client, "mobile_list_elements_on_screen", { device: id }),
    );
    assertVisibleElements(firstElements, "첫 실행");
    const screenSize = toolValue(
      await call(client, "mobile_get_screen_size", { device: id }),
    );

    const runDirectory = path.join(
      options.outputDirectory,
      new Date().toISOString().replaceAll(/[:.]/g, "-"),
    );
    await mkdir(runDirectory, { recursive: true });
    const firstScreenshot = path.join(runDirectory, "01-cold-start.png");
    await call(client, "mobile_save_screenshot", {
      device: id,
      saveTo: firstScreenshot,
    });

    await call(client, "mobile_press_button", {
      device: id,
      button: "BACK",
    });
    await delay(500);
    await call(client, "mobile_launch_app", {
      device: id,
      packageName: options.packageName,
      locale: "ko-KR",
    });
    await delay(2_000);
    const restoredElements = toolValue(
      await call(client, "mobile_list_elements_on_screen", { device: id }),
    );
    assertVisibleElements(restoredElements, "뒤로 가기 후 재실행");
    const restoredScreenshot = path.join(runDirectory, "02-restored.png");
    await call(client, "mobile_save_screenshot", {
      device: id,
      saveTo: restoredScreenshot,
    });

    console.log(
      JSON.stringify(
        {
          status: "passed",
          device: deviceSummary(device),
          packageName: options.packageName,
          screenSize,
          checks: [
            "cold-start",
            "accessibility-tree",
            "native-back",
            "relaunch",
          ],
          artifacts: [firstScreenshot, restoredScreenshot],
        },
        null,
        2,
      ),
    );
  } finally {
    await client.close();
  }
}

function toolText(result) {
  return (
    result?.content
      ?.filter((item) => item?.type === "text" && typeof item.text === "string")
      .map((item) => item.text)
      .join("\n")
      .trim() ?? ""
  );
}

function isVirtualDevice(device) {
  const text = JSON.stringify(device).toLowerCase();
  return (
    text.includes("emulator") ||
    text.includes("simulator") ||
    String(deviceId(device)).startsWith("emulator-")
  );
}

function deviceSummary(device) {
  return {
    id: deviceId(device),
    platform: device.platform ?? device.os ?? "unknown",
    type: device.type ?? (isVirtualDevice(device) ? "virtual" : "real"),
    name: device.name ?? device.model ?? "unknown",
  };
}

function assertVisibleElements(value, step) {
  const serialized = JSON.stringify(value);
  if (
    !serialized ||
    serialized === '""' ||
    serialized === "{}" ||
    serialized.includes('"elements":[]') ||
    /no (visible |accessible )?elements/i.test(serialized)
  ) {
    throw new Error(`${step} 화면에서 접근 가능한 요소를 찾지 못했습니다.`);
  }
}

async function call(client, name, args) {
  return client.callTool({ name, arguments: args });
}

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

const requiredTools = [
  "mobile_list_available_devices",
  "mobile_list_apps",
  "mobile_launch_app",
  "mobile_terminate_app",
  "mobile_list_elements_on_screen",
  "mobile_get_screen_size",
  "mobile_save_screenshot",
  "mobile_press_button",
];

const isDirectRun =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isDirectRun) {
  main().catch((error) => {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  });
}
