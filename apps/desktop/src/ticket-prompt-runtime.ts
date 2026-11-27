import { createContext } from "react";
import type { ProjectInflowItem } from "./api/googleChat";
import {
  inflowTicketPromptSource,
  promptTickets,
  type PromptSkill,
  type TicketDraftRequest,
  type TicketDraftResponse,
  type TicketPromptProject,
  type TicketPromptSource,
} from "./ticket-prompt";

type GenerateTicketDraft = (
  request: TicketDraftRequest,
  signal: AbortSignal,
) => Promise<TicketDraftResponse>;

type TicketDraftRevision = {
  project?: TicketPromptProject;
  text: string;
  skills: readonly PromptSkill[];
  notices: readonly string[];
  revision: number | null;
  manualPair?: boolean;
};

export type SavedTicketDraft = TicketDraftRevision & {
  previous?: TicketDraftRevision;
};

export function replaceTicketDraftRevision(
  current: SavedTicketDraft,
  next: TicketDraftRevision,
): SavedTicketDraft {
  const { previous: _older, ...previous } = current;
  return { ...next, previous };
}

export function createTicketPromptSession() {
  // Private drafts and their lineage stay in the current app session.
  const saved = new Map<string, readonly SavedTicketDraft[]>();
  const promoted = new Map<string, TicketPromptSource>();
  const key = (source: string, ticket: string) => `${source}:${ticket}`;
  return {
    read: (source: string, ticket: string) =>
      saved.get(key(source, ticket)) ?? [],
    write: (
      source: string,
      ticket: string,
      drafts: readonly SavedTicketDraft[],
    ) => saved.set(key(source, ticket), drafts),
    rememberPromotion(taskId: string, item: ProjectInflowItem) {
      if (promptTickets(item).length)
        promoted.set(taskId, inflowTicketPromptSource(item));
    },
    sourceForTask: (taskId: string) => promoted.get(taskId),
    clear() {
      saved.clear();
      promoted.clear();
    },
  };
}

export const TicketPromptRuntime = createContext<{
  generate: GenerateTicketDraft;
  session: ReturnType<typeof createTicketPromptSession>;
} | null>(null);
