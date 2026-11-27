import { createContext } from "react";
import type { TicketDraftRequest, TicketDraftResponse } from "./ticket-prompt";

type GenerateTicketDraft = (
  request: TicketDraftRequest,
  signal: AbortSignal,
) => Promise<TicketDraftResponse>;

export const TicketPromptRuntime = createContext<GenerateTicketDraft | null>(
  null,
);
