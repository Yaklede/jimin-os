import { Clipboard, FileText } from "lucide-react";
import { useContext, useEffect, useId, useMemo, useRef, useState } from "react";

import type { ProjectInflowItem } from "../api/googleChat";
import type { Task } from "../api/planning";
import { ticketPromptCopy as c } from "../copy/ticketPrompt";
import {
  buildTicketPrompt,
  inflowTicketPromptSource,
  promptTickets,
  projectPromptSkills,
  ticketDraftRequest,
  ticketPromptProjects,
  taskTicketPromptSource,
  type TicketPromptProject,
  type TicketPromptSource,
} from "../ticket-prompt";
import { readTicketSkillCatalog } from "../ticket-skill-catalog";
import {
  TicketPromptRuntime,
  type SavedTicketDraft,
} from "../ticket-prompt-runtime";
import "./ticketPromptDraft.css";

export function TaskTicketPrompt({
  task,
  linkedItem,
  collapsed = false,
  revealSavedDraft = false,
}: {
  task: Task;
  linkedItem?: ProjectInflowItem;
  collapsed?: boolean;
  revealSavedDraft?: boolean;
}) {
  const runtime = useContext(TicketPromptRuntime);
  const source = taskTicketPromptSource(
    task,
    runtime?.session.sourceForTask(task.id) ??
      (linkedItem ? inflowTicketPromptSource(linkedItem) : undefined),
  );
  if (!promptTickets(source).length) return null;
  const content = (
    <TicketPromptDraft
      key={task.id}
      source={source}
      revealSavedDraft={revealSavedDraft}
    />
  );
  return collapsed ? (
    <details className="ticket-prompt-draft-disclosure">
      <summary className="focus-visible-control">{c.openTaskPrompts}</summary>
      {content}
    </details>
  ) : (
    content
  );
}

export function TicketPromptDraft({
  source,
  revealSavedDraft = false,
}: {
  source: TicketPromptSource;
  revealSavedDraft?: boolean;
}) {
  const runtime = useContext(TicketPromptRuntime);
  const tickets = useMemo(() => promptTickets(source), [source]);
  const id = useId();
  const ticketSelect = useRef<HTMLSelectElement>(null);
  const generateButton = useRef<HTMLButtonElement>(null);
  const editor = useRef<HTMLTextAreaElement>(null);
  const copyButton = useRef<HTMLButtonElement>(null);
  const focusEditor = useRef(false);
  const returningToSelection = useRef(false);
  const operation = useRef<AbortController | null>(null);
  function readDrafts(ticketId: string) {
    const primary = runtime?.session.read(source.key, ticketId) ?? [];
    return !primary.length && source.fallbackKey
      ? (runtime?.session.read(source.fallbackKey, ticketId) ?? [])
      : primary;
  }
  const [initialSaved] = useState(() => {
    if (revealSavedDraft) {
      for (const ticket of tickets) {
        const drafts = readDrafts(ticket.id);
        if (drafts.length) return { ticketId: ticket.id, drafts };
      }
    }
  });
  const [selectedTicketId, setSelectedTicketId] = useState(
    initialSaved?.ticketId ?? "",
  );
  const [projectOverride, setProjectOverride] = useState<TicketPromptProject>();
  const [drafts, setDrafts] = useState<readonly SavedTicketDraft[]>(
    initialSaved?.drafts ?? [],
  );
  const [draftProject, setDraftProject] = useState<TicketPromptProject>();
  const [busy, setBusy] = useState<"generate" | "copy">();
  const [feedback, setFeedback] = useState<
    "copied" | "copyFailed" | "generateFailed"
  >();
  const activeDraft =
    drafts.find(({ project }) => project === draftProject) ?? drafts[0];
  const draft = activeDraft?.text;
  const draftOpen = Boolean(activeDraft);
  const ticket =
    tickets.length === 1
      ? tickets[0]
      : tickets.find((value) => value.id === selectedTicketId);
  const project = ticket?.project ?? projectOverride;
  const variants = ticket?.combinedProjects ?? [project];
  const cached = ticket ? readDrafts(ticket.id) : [];
  const storedDrafts = draftOpen ? drafts : cached;
  const missing = variants.filter(
    (variant) => !storedDrafts.some((entry) => entry.project === variant),
  );
  const analysisReady = source.analysisStatus === "ready";

  useEffect(() => () => operation.current?.abort(), []);
  useEffect(() => {
    if (draftOpen && focusEditor.current) {
      focusEditor.current = false;
      editor.current?.focus();
    } else if (!draftOpen && returningToSelection.current) {
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
    if (!ticket || !analysisReady || !missing.length || operation.current)
      return;
    const run = new AbortController();
    operation.current = run;
    setBusy("generate");
    try {
      const catalog = await readTicketSkillCatalog();
      if (run.signal.aborted) return;
      const names = catalog.skills.map(({ name }) => name);
      const results = [...storedDrafts];
      let failed = false;
      for (const variant of missing) {
        try {
          if (useAi && !runtime) throw new Error(c.generateFailed);
          const request = ticketDraftRequest(
            source,
            ticket,
            catalog.skills,
            variant,
          );
          // The AI route has one active slot; simultaneous requests would reject the second draft.
          const ai = useAi
            ? await runtime?.generate(request, run.signal)
            : undefined;
          if (run.signal.aborted) return;
          const skills = projectPromptSkills(variant, names);
          const allowed = new Set(request.candidates.map(({ name }) => name));
          skills.push(
            ...(ai?.skills ?? [])
              .filter(({ name }) => allowed.delete(name))
              .slice(0, 2),
          );
          const notices = !ai ? [c.fallbackDraft] : [];
          if (catalog.source === "unavailable")
            notices.push(c.catalogUnavailable);
          if (catalog.source === "snapshot" && skills.length)
            notices.push(c.catalogSnapshot);
          if (
            catalog.source !== "unavailable" &&
            variant &&
            !names.includes(ticketPromptProjects[variant].skill)
          )
            notices.push(c.projectSkillUnavailable);
          results.push({
            project: variant,
            text: buildTicketPrompt(source, ticket, variant, skills, ai),
            skills,
            notices,
            revision: source.revision,
          });
          runtime?.session.write(source.key, ticket.id, [...results]);
        } catch {
          if (run.signal.aborted) return;
          failed = true;
        }
      }
      setDrafts(results);
      if (!draftOpen && results.length) {
        focusEditor.current = true;
        setDraftProject(results[0]?.project);
      }
      setFeedback(failed ? "generateFailed" : undefined);
    } catch {
      if (!run.signal.aborted) setFeedback("generateFailed");
    } finally {
      operation.current = null;
      if (!run.signal.aborted) setBusy(undefined);
    }
  }

  function closeDraft() {
    returningToSelection.current = true;
    setDrafts([]);
    setFeedback(undefined);
  }

  function editDraft(text: string) {
    if (!ticket || !activeDraft) return;
    const next = drafts.map((entry) =>
      entry === activeDraft ? { ...entry, text } : entry,
    );
    runtime?.session.write(source.key, ticket.id, next);
    setDrafts(next);
    if (feedback !== "generateFailed") setFeedback(undefined);
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

  const retry = missing.length > 0 &&
    (feedback === "generateFailed" ||
      (draftOpen && ticket?.combinedProjects)) && (
      <div className="ticket-prompt-draft__recovery">
        {feedback === "generateFailed" && (
          <p role="alert">{draftOpen ? c.partialFailure : c.generateFailed}</p>
        )}
        <div className="ticket-prompt-draft__actions">
          {draftOpen && (
            <button
              type="button"
              className="secondary-button focus-visible-control"
              disabled={!analysisReady || Boolean(busy)}
              onClick={() => void generate(true)}
            >
              {busy === "generate" ? c.generating : c.generateMissing}
            </button>
          )}
          <button
            type="button"
            className="secondary-button focus-visible-control"
            disabled={!analysisReady || Boolean(busy)}
            onClick={() => void generate(false)}
          >
            {c.useBasicDraft}
          </button>
        </div>
      </div>
    );

  return (
    <section
      className="ticket-prompt-draft"
      aria-labelledby={`${id}-title`}
      aria-busy={Boolean(busy)}
    >
      <header>
        <h3 id={`${id}-title`}>{c.title}</h3>
        <p>{draftOpen ? c.draftHelp : c.help}</p>
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
          {ticket?.combinedProjects && <p>{c.combinedHint}</p>}
          {ticket && !ticket.project && !ticket.combinedProjects && (
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
              {source.analysisStatus === "failed"
                ? c.analysisFailed
                : c.analysisPending}
            </p>
          )}
          <div className="ticket-prompt-draft__actions">
            {cached.length > 0 && (
              <button
                ref={generateButton}
                type="button"
                className="secondary-button focus-visible-control"
                disabled={Boolean(busy)}
                onClick={() => {
                  focusEditor.current = true;
                  setDraftProject(cached[0]?.project);
                  setDrafts(cached);
                }}
              >
                <FileText aria-hidden="true" /> {c.openSavedDraft}
              </button>
            )}
            {(!cached.length || missing.length > 0) && (
              <button
                ref={!cached.length ? generateButton : undefined}
                type="button"
                className="secondary-button focus-visible-control"
                disabled={!ticket || !analysisReady || Boolean(busy)}
                onClick={() => void generate(true)}
              >
                <FileText aria-hidden="true" />
                {busy === "generate"
                  ? c.generating
                  : cached.length
                    ? c.generateMissing
                    : ticket?.combinedProjects
                      ? c.generateBoth
                      : c.generate}
              </button>
            )}
          </div>
          {retry}
        </>
      ) : (
        <>
          {drafts.length > 1 && (
            <div
              role="group"
              aria-labelledby={`${id}-variants`}
              className="ticket-prompt-draft__variants"
            >
              <span id={`${id}-variants`}>{c.variant}</span>
              <div className="ticket-prompt-draft__actions">
                {drafts.map(({ project }) => (
                  <button
                    key={project ?? "general"}
                    type="button"
                    className="secondary-button focus-visible-control"
                    aria-pressed={activeDraft?.project === project}
                    disabled={Boolean(busy)}
                    onClick={() => {
                      setDraftProject(project);
                      if (feedback !== "generateFailed") setFeedback(undefined);
                    }}
                  >
                    {project
                      ? ticketPromptProjects[project].label
                      : c.generalDraft}
                  </button>
                ))}
              </div>
            </div>
          )}
          {activeDraft?.notices.map((notice) => (
            <p key={notice}>{notice}</p>
          ))}
          {activeDraft?.revision !== source.revision && (
            <p>{c.newerAnalysis}</p>
          )}
          {Boolean(activeDraft?.skills.length) && (
            <div>
              <strong>{c.skills}</strong>
              <div role="list" className="ticket-prompt-draft__skills">
                {activeDraft?.skills.map(({ name, reason }) => (
                  <div key={name} role="listitem">
                    <code>${name}</code>
                    <small>{reason}</small>
                  </div>
                ))}
              </div>
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
              onChange={(event) => editDraft(event.currentTarget.value)}
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
          {feedback && feedback !== "generateFailed" && (
            <p role={feedback === "copied" ? "status" : "alert"}>
              {c[feedback]}
            </p>
          )}
          {retry}
        </>
      )}
    </section>
  );
}
