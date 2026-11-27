import { Clipboard, FileText } from "lucide-react";
import { useContext, useEffect, useId, useMemo, useRef, useState } from "react";

import type { ProjectInflowItem } from "../api/googleChat";
import { ticketPromptCopy as c } from "../copy/ticketPrompt";
import {
  buildTicketPrompt,
  promptTickets,
  projectPromptSkills,
  ticketDraftRequest,
  ticketPromptProjects,
  type PromptSkill,
  type TicketPromptProject,
} from "../ticket-prompt";
import { readTicketSkillCatalog } from "../ticket-skill-catalog";
import { TicketPromptRuntime } from "../ticket-prompt-runtime";
import "./ticketPromptDraft.css";

export function TicketPromptDraft({ item }: { item: ProjectInflowItem }) {
  const generateDraft = useContext(TicketPromptRuntime);
  const tickets = useMemo(() => promptTickets(item), [item]);
  const id = useId();
  const ticketSelect = useRef<HTMLSelectElement>(null);
  const generateButton = useRef<HTMLButtonElement>(null);
  const editor = useRef<HTMLTextAreaElement>(null);
  const copyButton = useRef<HTMLButtonElement>(null);
  const returningToSelection = useRef(false);
  const operation = useRef<AbortController | null>(null);
  const [selectedTicketId, setSelectedTicketId] = useState("");
  const [projectOverride, setProjectOverride] = useState<TicketPromptProject>();
  const [draft, setDraft] = useState<string>();
  const [skills, setSkills] = useState<PromptSkill[]>([]);
  const [notices, setNotices] = useState<string[]>([]);
  const [busy, setBusy] = useState<"generate" | "copy">();
  const [feedback, setFeedback] = useState<
    "copied" | "copyFailed" | "generateFailed"
  >();
  const [draftRevision, setDraftRevision] = useState<number | null>();
  const draftOpen = draft !== undefined;
  const ticket =
    tickets.length === 1
      ? tickets[0]
      : tickets.find((value) => value.id === selectedTicketId);
  const project = ticket?.project ?? projectOverride;
  const analysisReady =
    item.analysisStatus === "ready" &&
    (item.sourceRevision == null ||
      item.analyzedRevision === item.sourceRevision);

  useEffect(() => () => operation.current?.abort(), []);
  useEffect(() => {
    if (draftOpen) editor.current?.focus();
    else if (returningToSelection.current) {
      returningToSelection.current = false;
      (ticketSelect.current ?? generateButton.current)?.focus();
    }
  }, [draftOpen]);
  useEffect(() => {
    if (!busy && document.activeElement === document.body) {
      if (feedback === "copied")
        copyButton.current?.focus({ preventScroll: true });
      if (feedback === "generateFailed")
        generateButton.current?.focus({ preventScroll: true });
    }
  }, [busy, feedback]);

  if (!tickets.length && !draftOpen) return null;

  async function generate(useAi: boolean) {
    if (!ticket || !analysisReady || operation.current) return;
    const run = new AbortController();
    operation.current = run;
    setBusy("generate");
    try {
      const catalog = await readTicketSkillCatalog();
      if (run.signal.aborted) return;
      const names = catalog.skills.map(({ name }) => name);
      const recommended = projectPromptSkills(project, names);
      if (useAi && !generateDraft) throw new Error(c.generateFailed);
      const request = ticketDraftRequest(item, ticket, catalog.skills);
      const ai = useAi ? await generateDraft?.(request, run.signal) : undefined;
      if (run.signal.aborted) return;
      // The API enforces this too; only installed candidates can become skill calls.
      const allowed = new Set(request.candidates.map(({ name }) => name));
      recommended.push(
        ...(ai?.skills ?? [])
          .filter(({ name }) => allowed.delete(name))
          .slice(0, 2),
      );
      const notice =
        catalog.source === "unavailable"
          ? [c.catalogUnavailable]
          : catalog.source === "snapshot" && recommended.length > 0
            ? [c.catalogSnapshot]
            : [];
      if (
        catalog.source !== "unavailable" &&
        project &&
        !names.includes(ticketPromptProjects[project].skill)
      ) {
        notice.push(c.projectSkillUnavailable);
      }
      if (!ai) notice.unshift(c.fallbackDraft);
      setFeedback(undefined);
      setNotices(notice);
      setSkills(recommended);
      setDraftRevision(item.sourceRevision);
      setDraft(buildTicketPrompt(item, ticket, project, recommended, ai));
    } catch {
      if (!run.signal.aborted) setFeedback("generateFailed");
    } finally {
      operation.current = null;
      if (!run.signal.aborted) setBusy(undefined);
    }
  }

  function closeDraft() {
    returningToSelection.current = true;
    setDraft(undefined);
    setFeedback(undefined);
  }

  async function copyDraft() {
    if (!draft?.trim() || operation.current) return;
    const run = new AbortController();
    operation.current = run;
    setBusy("copy");
    setFeedback(undefined);
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(draft);
      } else {
        editor.current?.focus();
        editor.current?.select();
        if (!document.execCommand("copy")) throw new Error();
        copyButton.current?.focus({ preventScroll: true });
      }
      if (!run.signal.aborted) setFeedback("copied");
    } catch {
      if (!run.signal.aborted) {
        editor.current?.focus();
        editor.current?.select();
        setFeedback("copyFailed");
      }
    } finally {
      operation.current = null;
      if (!run.signal.aborted) setBusy(undefined);
    }
  }

  return (
    <section
      className="ticket-prompt-draft"
      aria-labelledby={`${id}-title`}
      aria-busy={Boolean(busy)}
    >
      <header>
        <h3 id={`${id}-title`}>{c.title}</h3>
        <p>{c.help}</p>
      </header>
      {!draftOpen ? (
        <>
          {tickets.length > 1 && (
            <label>
              <span>{c.chooseTicket}</span>
              <select
                ref={ticketSelect}
                className="focus-visible-control"
                value={ticket?.id ?? ""}
                disabled={Boolean(busy)}
                onChange={(event) => {
                  setSelectedTicketId(event.currentTarget.value);
                  setProjectOverride(undefined);
                }}
              >
                <option value="">{c.chooseTicketHint}</option>
                {tickets.map((value) => (
                  <option key={value.id} value={value.id}>
                    #{value.id}
                    {value.title ? ` · ${value.title}` : ""}
                  </option>
                ))}
              </select>
            </label>
          )}
          {ticket && !ticket.project && (
            <label>
              <span>{c.project}</span>
              <select
                className="focus-visible-control"
                value={projectOverride ?? ""}
                disabled={Boolean(busy)}
                aria-describedby={`${id}-project-help`}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setProjectOverride(
                    Object.hasOwn(ticketPromptProjects, value)
                      ? (value as TicketPromptProject)
                      : undefined,
                  );
                }}
              >
                <option value="">{c.general}</option>
                {Object.entries(ticketPromptProjects).map(([value, info]) => (
                  <option key={value} value={value}>
                    {info.label}
                  </option>
                ))}
              </select>
              <small id={`${id}-project-help`}>{c.projectUnknown}</small>
            </label>
          )}
          {!analysisReady && (
            <p role="status">
              {item.analysisStatus === "failed"
                ? c.analysisFailed
                : c.analysisPending}
            </p>
          )}
          <button
            ref={generateButton}
            type="button"
            className="secondary-button focus-visible-control"
            disabled={!ticket || !analysisReady || Boolean(busy)}
            onClick={() => void generate(true)}
          >
            <FileText aria-hidden="true" />
            {busy === "generate" ? c.generating : c.generate}
          </button>
          {feedback === "generateFailed" && (
            <>
              <p role="alert">{c.generateFailed}</p>
              <button
                type="button"
                className="secondary-button focus-visible-control"
                disabled={!ticket || !analysisReady || Boolean(busy)}
                onClick={() => void generate(false)}
              >
                {c.useBasicDraft}
              </button>
            </>
          )}
        </>
      ) : (
        <>
          {notices.map((notice) => (
            <p key={notice}>{notice}</p>
          ))}
          {draftRevision !== item.sourceRevision && <p>{c.newerAnalysis}</p>}
          {skills.length > 0 && (
            <div>
              <strong>{c.skills}</strong>
              <ul>
                {skills.map(({ name, reason }) => (
                  <li key={name}>
                    <code>${name}</code> · {reason}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <label>
            <span>{c.editor}</span>
            <textarea
              ref={editor}
              className="focus-visible-control"
              value={draft}
              rows={8}
              spellCheck={false}
              onChange={(event) => {
                setDraft(event.currentTarget.value);
                setFeedback(undefined);
              }}
            />
          </label>
          <div className="ticket-prompt-draft__actions">
            <button
              ref={copyButton}
              type="button"
              className="secondary-button focus-visible-control"
              disabled={!draft?.trim() || Boolean(busy)}
              onClick={() => void copyDraft()}
            >
              <Clipboard aria-hidden="true" />
              {busy === "copy" ? c.copying : c.copy}
            </button>
            <button
              type="button"
              className="secondary-button focus-visible-control"
              disabled={Boolean(busy)}
              onClick={closeDraft}
            >
              {c.closeDraft}
            </button>
          </div>
          {feedback && (
            <p role={feedback === "copied" ? "status" : "alert"}>
              {c[feedback]}
            </p>
          )}
        </>
      )}
    </section>
  );
}
