import { describe, expect, it } from "vitest";
import fixtures from "../../../tests/fixtures/reviewed-assignment-messages.json";
import { reviewedAssignmentPreview } from "./assignmentPreview";

describe("reviewed assignment preview", () => {
  for (const fixture of fixtures) {
    it(`matches the Rust outbound contract: ${fixture.title}`, () => {
      expect(reviewedAssignmentPreview(fixture)).toEqual({
        message: fixture.message,
        truncated: false,
      });
    });
  }
  it("updates edited content immediately, including edits after 400 characters", () => {
    const preview = reviewedAssignmentPreview({
      ...fixtures[0],
      content: "가".repeat(500) + "\n수정한 마지막 요청",
    });
    expect(preview.message).toContain("수정한 마지막 요청");
    expect(preview.message).not.toContain(fixtures[0].content);
    expect(preview.truncated).toBe(false);
  });
  it("shows the bounded body and keeps reference links when content is too long", () => {
    const preview = reviewedAssignmentPreview({
      ...fixtures[0],
      content: "📝".repeat(2000),
    });
    expect(preview.truncated).toBe(true);
    expect(Array.from(preview.message).length).toBeLessThanOrEqual(1600);
    expect(preview.message).toContain(
      "…\n\n관련 자료\n- https://itsm.example.test/issues/4245",
    );
  });
  it("keeps an intentionally empty description empty instead of restoring AI text", () => {
    const preview = reviewedAssignmentPreview({ ...fixtures[0], content: "" });
    expect(preview.message).not.toContain("업무 내용");
    expect(preview.message).not.toContain(fixtures[0].content);
    expect(preview.truncated).toBe(false);
  });
});
