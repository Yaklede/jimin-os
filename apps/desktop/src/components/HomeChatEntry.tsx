import { List, Send } from "lucide-react";
import {
  type Dispatch,
  type FormEvent,
  type SetStateAction,
  useId,
  useRef,
} from "react";

import { copy } from "../copy";
import { homeChatEntryCopy as c } from "../copy/homeChatEntry";
import { createUuidV7 } from "../uuid";

export type HomeChatDraft = {
  text: string;
  messageId?: string;
  sending?: boolean;
  error?: boolean;
};

export function HomeChatEntry({
  ready,
  draft,
  setDraft,
  onOpenHistory,
  onSend,
}: {
  ready: boolean;
  draft: HomeChatDraft;
  setDraft: Dispatch<SetStateAction<HomeChatDraft>>;
  onOpenHistory(): void;
  onSend(text: string, messageId: string): Promise<boolean>;
}) {
  const id = useId();
  const { sending = false, error = false } = draft;
  const sendingRef = useRef(false);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const text = draft.text.trim();
    if (!text || sending || sendingRef.current || !ready) return;
    const messageId = draft.messageId ?? createUuidV7();
    setDraft({ text: draft.text, messageId, sending: true });
    sendingRef.current = true;
    let accepted = false;
    try {
      accepted = await onSend(text, messageId);
    } catch {
      // A rejected send keeps the draft available for retry.
    } finally {
      sendingRef.current = false;
      setDraft((current) => {
        if (current.messageId !== messageId) return current;
        return accepted
          ? { text: "" }
          : { text: current.text, messageId, error: true };
      });
    }
  }

  return (
    <section
      className="home-command home-command--compact"
      aria-label={c.label}
    >
      <form
        className="home-command__form"
        aria-busy={sending}
        onSubmit={submit}
      >
        <button
          id="home-chat-history"
          className="home-chat-entry__history-button focus-visible-control"
          type="button"
          aria-label={c.history}
          title={c.history}
          onClick={onOpenHistory}
        >
          <List aria-hidden="true" />
        </button>
        <label className="sr-only" htmlFor="home-assistant-command">
          {c.label}
        </label>
        <input
          id="home-assistant-command"
          value={draft.text}
          maxLength={24_000}
          placeholder={ready ? c.inputHint : copy.home.commandNeedsConnection}
          disabled={!ready}
          readOnly={sending}
          aria-invalid={error || undefined}
          aria-describedby={error ? `${id}-error` : undefined}
          onChange={(event) => {
            setDraft({ text: event.target.value });
          }}
        />
        <button
          className="primary-button focus-visible-control"
          type="submit"
          disabled={!ready || sending || !draft.text.trim()}
          aria-label={copy.home.commandSend}
        >
          {sending ? (
            <span className="button-spinner" aria-hidden="true" />
          ) : (
            <Send aria-hidden="true" />
          )}
        </button>
      </form>
      {error && (
        <p id={`${id}-error`} className="inline-alert" role="alert">
          {copy.home.commandFailed}
        </p>
      )}
      {sending && (
        <p className="assistant-turn-feedback" role="status">
          {copy.home.requestSending}
        </p>
      )}
    </section>
  );
}
