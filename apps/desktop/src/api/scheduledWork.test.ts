import { afterEach, describe, expect, it, vi } from "vitest";
import { scheduledWorkTemplate } from "../components/ScheduledWorkPanel";
import {
  normalizeScheduledWorkDefinition,
  scheduledRequest,
  type ScheduledWorkMessageDetail,
} from "./scheduledWork";

afterEach(() => vi.unstubAllGlobals());

describe("scheduled reminder display settings", () => {
  it.each(["deadline", "followup", "brief"] as const)(
    "defaults the %s template and legacy definitions to titles only",
    (template) => {
      const definition = scheduledWorkTemplate("workspace", template);
      expect(definition.messageDetail).toBe("title_only");
      delete definition.messageDetail;
      const normalized = normalizeScheduledWorkDefinition(definition);
      expect(normalized.messageDetail).toBe("title_only");
      expect(definition.messageDetail).toBeUndefined();
      expect(normalized).toMatchObject(definition);
    },
  );

  it.each<ScheduledWorkMessageDetail>(["title_only", "title_and_details"])(
    "preserves %s in preview, save and readback",
    async (messageDetail) => {
      const definition = normalizeScheduledWorkDefinition({
        ...scheduledWorkTemplate("workspace"),
        messageDetail,
      });
      const fetchMock = vi.fn<typeof fetch>().mockImplementation(
        async () =>
          new Response(JSON.stringify({ id: "rule", definition, version: 2 }), {
            status: 200,
            headers: { "Content-Type": "application/json" },
          }),
      );
      vi.stubGlobal("fetch", fetchMock);
      await scheduledRequest(
        "http://localhost/api",
        "test-access",
        "/preview",
        definition,
        "POST",
      );
      await expect(
        scheduledRequest(
          "http://localhost/api",
          "test-access",
          "/rule",
          { definition, enabled: false, expectedVersion: 1 },
          "PUT",
        ),
      ).resolves.toMatchObject({ definition: { messageDetail } });
      expect(
        JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body)).messageDetail,
      ).toBe(messageDetail);
      expect(
        JSON.parse(String(fetchMock.mock.calls[1]?.[1]?.body)).definition
          .messageDetail,
      ).toBe(messageDetail);
      expect(
        JSON.parse(String(fetchMock.mock.calls[1]?.[1]?.body)).expectedVersion,
      ).toBe(1);
      expect(
        JSON.stringify({
          ...definition,
          messageDetail:
            messageDetail === "title_only" ? "title_and_details" : "title_only",
        }),
      ).not.toBe(JSON.stringify(definition));
    },
  );
});
