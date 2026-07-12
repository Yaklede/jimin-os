import { BrainCircuit, MessageCircleMore } from "lucide-react";

import { copy } from "../copy";

export function MemoryWorkspace({
  onOpenConversation,
}: {
  onOpenConversation(): void;
}) {
  return (
    <section className="memory-page">
      <header className="page-heading">
        <p>개인 맥락</p>
        <h1>{copy.memory.title}</h1>
        <span>{copy.memory.description}</span>
      </header>
      <section className="memory-empty" aria-labelledby="memory-empty-title">
        <span aria-hidden="true">
          <BrainCircuit />
        </span>
        <div>
          <h2 id="memory-empty-title">{copy.memory.emptyTitle}</h2>
          <p>{copy.memory.emptyDescription}</p>
        </div>
        <button
          className="primary-button focus-visible-control"
          type="button"
          onClick={onOpenConversation}
        >
          <MessageCircleMore aria-hidden="true" />
          {copy.memory.openConversation}
        </button>
      </section>
    </section>
  );
}
