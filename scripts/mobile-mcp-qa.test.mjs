import assert from "node:assert/strict";
import test from "node:test";

import {
  deviceId,
  devicesFrom,
  parseArguments,
  parseToolJson,
  selectSafeDevice,
  toolValue,
} from "./mobile-mcp-qa.mjs";

test("parses device payloads returned by Mobile MCP", () => {
  const devices = devicesFrom({
    content: [
      {
        type: "text",
        text: JSON.stringify({
          devices: [
            {
              id: "emulator-5554",
              platform: "android",
              type: "emulator",
            },
          ],
        }),
      },
    ],
  });

  assert.equal(devices.length, 1);
  assert.equal(deviceId(devices[0]), "emulator-5554");
});

test("supports the nested mobilecli device response", () => {
  const devices = devicesFrom({
    content: [
      {
        type: "text",
        text: JSON.stringify({
          data: {
            devices: [{ serial: "ios-sim-1", type: "simulator" }],
          },
        }),
      },
    ],
  });

  assert.equal(deviceId(devices[0]), "ios-sim-1");
});

test("refuses a real device for the default QA smoke", () => {
  assert.throws(
    () =>
      selectSafeDevice([
        { id: "R5KL20581QR", platform: "android", type: "real" },
      ]),
    /운영 실기기/,
  );
});

test("parses safe command defaults and overrides", () => {
  assert.deepEqual(parseArguments(["doctor"]).packageName, "io.jimin.os.dev");
  const options = parseArguments([
    "smoke",
    "--",
    "--device=emulator-5554",
    "--package=io.jimin.os.dev",
    "--output=.mobile-mcp/test-artifacts",
  ]);
  assert.equal(options.command, "smoke");
  assert.equal(options.device, "emulator-5554");
  assert.match(options.outputDirectory, /\.mobile-mcp\/test-artifacts$/);
  assert.throws(
    () => parseArguments(["smoke", "--package=io.jimin.os"]),
    /운영 패키지/,
  );
});

test("surfaces MCP tool errors instead of parsing them as results", () => {
  assert.throws(
    () =>
      parseToolJson({
        isError: true,
        content: [{ type: "text", text: "device unavailable" }],
      }),
    /device unavailable/,
  );
});

test("keeps human-readable Mobile MCP results available to smoke checks", () => {
  assert.equal(
    toolValue({
      content: [{ type: "text", text: "Found these apps: io.jimin.os.dev" }],
    }),
    "Found these apps: io.jimin.os.dev",
  );
});
