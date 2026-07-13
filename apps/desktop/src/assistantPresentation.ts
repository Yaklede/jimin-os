import {
  type AssistantPresentationItem,
  type ConversationMessage,
} from "./api/agent";

type TaskPresentationItem = Extract<
  AssistantPresentationItem,
  { type: "task" }
>;
type SchedulePresentationItem = Extract<
  AssistantPresentationItem,
  { type: "schedule" }
>;
type ProjectPresentationItem = Extract<
  AssistantPresentationItem,
  { type: "project" }
>;

export type AssistantPresentation =
  | {
      kind: "tasks";
      title: string;
      summary: string;
      items: TaskPresentationItem[];
      highlightedTaskId: string | undefined;
    }
  | {
      kind: "schedule";
      title: string;
      summary: string;
      items: SchedulePresentationItem[];
    }
  | {
      kind: "projects";
      title: string;
      summary: string;
      items: ProjectPresentationItem[];
    }
  | {
      kind: "summary";
      title: string;
      summary: string;
    };

/**
 * Converts the server-validated assistant surface into a render-only view
 * model. Intent classification and entity selection deliberately do not happen
 * in the client.
 */
export function presentationForMessage(
  message: ConversationMessage,
): AssistantPresentation {
  const summary = message.content.trim();
  const presentation = message.presentation;
  if (!presentation) {
    return {
      kind: "summary",
      title: "요청 결과",
      summary,
    };
  }

  switch (presentation.kind) {
    case "tasks": {
      const items = presentation.items.filter(
        (item): item is TaskPresentationItem => item.type === "task",
      );
      return {
        kind: "tasks",
        title: presentation.title,
        summary,
        items,
        highlightedTaskId: items[0]?.id,
      };
    }
    case "schedule":
      return {
        kind: "schedule",
        title: presentation.title,
        summary,
        items: presentation.items.filter(
          (item): item is SchedulePresentationItem => item.type === "schedule",
        ),
      };
    case "projects":
      return {
        kind: "projects",
        title: presentation.title,
        summary,
        items: presentation.items.filter(
          (item): item is ProjectPresentationItem => item.type === "project",
        ),
      };
    case "summary":
      return {
        kind: "summary",
        title: presentation.title,
        summary,
      };
  }
}
