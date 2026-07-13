import {
  type AssistantPresentationItem,
  type AssistantPresentationSection as ApiPresentationSection,
  type ConversationMessage,
} from "./api/agent";

export type TaskPresentationItem = Extract<
  AssistantPresentationItem,
  { type: "task" }
>;
export type SchedulePresentationItem = Extract<
  AssistantPresentationItem,
  { type: "schedule" }
>;
export type ProjectPresentationItem = Extract<
  AssistantPresentationItem,
  { type: "project" }
>;

export type AssistantPresentationSection =
  | {
      kind: "tasks";
      title: string;
      view: "list" | "checklist";
      items: TaskPresentationItem[];
    }
  | {
      kind: "schedule";
      title: string;
      view: "list" | "timeline";
      items: SchedulePresentationItem[];
    }
  | {
      kind: "projects";
      title: string;
      view: "list" | "cards";
      items: ProjectPresentationItem[];
    };

export type AssistantPresentation = {
  title: string;
  summary: string;
  layout: "stack" | "split" | "focus";
  sections: AssistantPresentationSection[];
  focusItemId: string | undefined;
};

/**
 * Converts the server-validated assistant surface into a render-only view
 * model. The client never classifies intent or accepts generated markup.
 */
export function presentationForMessage(
  message: ConversationMessage,
): AssistantPresentation {
  const summary = message.content.trim();
  const presentation = message.presentation;
  if (!presentation) {
    return {
      title: "요청 결과",
      summary,
      layout: "stack",
      sections: [],
      focusItemId: undefined,
    };
  }

  const itemsById = new Map(
    presentation.items.map((item) => [item.id, item] as const),
  );
  const sourceSections = presentation.sections.length
    ? presentation.sections
    : legacySections(presentation.kind, presentation.title, presentation.items);
  const sections = sourceSections
    .map((section) => sectionForRender(section, itemsById))
    .filter(
      (section): section is AssistantPresentationSection =>
        section !== undefined && section.items.length > 0,
    );
  const selectedIds = new Set(
    sections.flatMap((section) => section.items.map((item) => item.id)),
  );
  const requestedFocus = presentation.focusItemId ?? undefined;

  return {
    title: presentation.title,
    summary,
    layout: sections.length ? presentation.layout : "stack",
    sections,
    focusItemId:
      requestedFocus && selectedIds.has(requestedFocus)
        ? requestedFocus
        : sections[0]?.items[0]?.id,
  };
}

function legacySections(
  kind: "summary" | "tasks" | "schedule" | "projects" | "composite",
  title: string,
  items: AssistantPresentationItem[],
): ApiPresentationSection[] {
  if (kind === "summary" || kind === "composite") return [];
  return [
    {
      kind,
      title,
      view: "list",
      itemIds: items.map((item) => item.id),
    },
  ];
}

function sectionForRender(
  section: ApiPresentationSection,
  itemsById: Map<string, AssistantPresentationItem>,
): AssistantPresentationSection | undefined {
  const items = section.itemIds
    .map((id) => itemsById.get(id))
    .filter((item): item is AssistantPresentationItem => item !== undefined);
  if (section.kind === "tasks") {
    return {
      kind: "tasks",
      title: section.title,
      view: section.view === "checklist" ? "checklist" : "list",
      items: items.filter(
        (item): item is TaskPresentationItem => item.type === "task",
      ),
    };
  }
  if (section.kind === "schedule") {
    return {
      kind: "schedule",
      title: section.title,
      view: section.view === "timeline" ? "timeline" : "list",
      items: items.filter(
        (item): item is SchedulePresentationItem => item.type === "schedule",
      ),
    };
  }
  return {
    kind: "projects",
    title: section.title,
    view: section.view === "cards" ? "cards" : "list",
    items: items.filter(
      (item): item is ProjectPresentationItem => item.type === "project",
    ),
  };
}
