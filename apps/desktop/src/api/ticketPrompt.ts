import { AgentRequestError } from "./agent";
import type { TicketDraftRequest, TicketDraftResponse } from "../ticket-prompt";

export async function requestTicketDraft(
  baseUrl: string,
  access: string,
  request: TicketDraftRequest,
  signal: AbortSignal,
): Promise<TicketDraftResponse> {
  const response = await fetch(
    `${baseUrl.replace(/\/+$/, "")}/v1/ticket-prompt/draft`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${access}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(request),
      cache: "no-store",
      signal,
    },
  );
  if (response.status === 401) throw new AgentRequestError("unauthorized");
  if (!response.ok) throw new AgentRequestError("unavailable");
  return response.json();
}
