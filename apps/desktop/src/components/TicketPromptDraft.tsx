import { Clipboard, FileText, RefreshCw } from "lucide-react";
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
  replaceTicketDraftRevision,
  TicketPromptRuntime,
  type SavedTicketDraft,
} from "../ticket-prompt-runtime";
import "./ticketPromptDraft.css";

const manualPair = ["biscuitlink", "alldaypay"] as const;
type ProjectSkillChoice = TicketPromptProject | "both" | "";

function missingDraftProjects(
  projects: readonly (TicketPromptProject | undefined)[],
  drafts: readonly SavedTicketDraft[],
) {
  return projects.filter(
    (project) => !drafts.some((draft) => draft.project === project),
  );
}

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
  const regenerateButton = useRef<HTMLButtonElement>(null);
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
  const [projectOverride, setProjectOverride] = useState<ProjectSkillChoice>();
  const [regenerateProject, setRegenerateProject] =
    useState<ProjectSkillChoice>();
  const [drafts, setDrafts] = useState<readonly SavedTicketDraft[]>(
    initialSaved?.drafts ?? [],
  );
  const [draftProject, setDraftProject] = useState<TicketPromptProject>();
  const [busy, setBusy] = useState<"generate" | "regenerate" | "copy">();
  const [feedback, setFeedback] = useState<
    | "copied"
    | "copyFailed"
    | "generateFailed"
    | "regenerated"
    | "generatedPair"
    | "regenerateFailed"
    | "restoredDraft"
  >();
  const activeDraft =
    drafts.find(({ project }) => project === draftProject) ?? drafts[0];
  const draft = activeDraft?.text;
  const draftOpen = Boolean(activeDraft);
  const ticket =
    tickets.length === 1
      ? tickets[0]
      : tickets.find((value) => value.id === selectedTicketId);
  const canChooseProject = ticket && !ticket.project && !ticket.combinedProjects;
  const canChooseRegeneration = canChooseProject && drafts.length === 1;
  const regenerationScope =
    (canChooseRegeneration ? regenerateProject : undefined) ??
    activeDraft?.project;
  const cached = ticket ? readDrafts(ticket.id) : [];
  const storedDrafts = draftOpen ? drafts : cached;
  const chosenProject =
    ticket?.project ??
    projectOverride ??
    (storedDrafts.some(({ manualPair: paired }) => paired)
      ? "both"
      : storedDrafts[0]?.project);
  const pairRequested = Boolean(
    ticket?.combinedProjects || chosenProject === "both",
  );
  const variants =
    ticket?.combinedProjects ??
    (chosenProject === "both" ? manualPair : [chosenProject || undefined]);
  const missing = missingDraftProjects(variants, storedDrafts);
  const analysisReady = source.analysisStatus === "ready";
  const regenerateLabel =
    regenerationScope === "both"
      ? activeDraft?.project
        ? c.addOtherDraft
        : c.generateBoth
      : c.regenerate;

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
        (generateButton.current ?? regenerateButton.current)?.focus({
          preventScroll: true,
        });
      if (
        feedback === "regenerated" ||
        feedback === "generatedPair" ||
        feedback === "regenerateFailed" ||
        feedback === "restoredDraft"
      )
        regenerateButton.current?.focus({ preventScroll: true });
    }
  }, [busy, feedback]);

  if (!tickets.length && !draftOpen) return null;

  async function generate(
    useAi: boolean,
    replacement?: { current: SavedTicketDraft; choice?: ProjectSkillChoice },
  ) {
    const pairFromEditor = replacement?.choice === "both";
    if (!ticket || !analysisReady || operation.current) return;
    const targets = !replacement
      ? missing
      : replacement.choice === "both"
        ? missingDraftProjects(manualPair, storedDrafts)
        : [replacement.choice || undefined];
    const replacementIndex = replacement
      ? storedDrafts.indexOf(replacement.current)
      : -1;
    if (!targets.length || (replacement && replacementIndex < 0)) return;
    const expandingPair = pairFromEditor || (!replacement && pairRequested);
    const manuallyPaired =
      pairFromEditor || (!replacement && chosenProject === "both");
    const failure = replacement ? "regenerateFailed" : "generateFailed";
    const run = new AbortController();
    operation.current = run;
    setBusy(replacement ? "regenerate" : "generate");
    if (replacement) setFeedback(undefined);
    try {
      const catalog = await readTicketSkillCatalog();
      if (run.signal.aborted) return;
      const names = catalog.skills.map(({ name }) => name);
      const results = [...storedDrafts];
      // A general draft becomes the first successful scoped draft, but remains undoable.
      // An existing project draft is not rewritten when only its partner is missing.
      let replaceIndex = expandingPair
        ? results.findIndex(({ project }) => project === undefined)
        : replacementIndex;
      let replacedProject: TicketPromptProject | undefined;
      let succeeded = 0;
      for (const variant of targets) {
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
          const generated: SavedTicketDraft = {
            project: variant,
            text: buildTicketPrompt(source, ticket, variant, skills, ai),
            skills,
            notices,
            revision: source.revision,
            manualPair: manuallyPaired || replacement?.current.manualPair,
          };
          if (replaceIndex >= 0) {
            const previous = results[replaceIndex];
            results[replaceIndex] = replaceTicketDraftRevision(
              previous,
              generated,
            );
            if (previous === activeDraft) replacedProject = variant;
            replaceIndex = -1;
          } else results.push(generated);
          succeeded++;
          runtime?.session.write(source.key, ticket.id, [...results]);
        } catch {
          if (run.signal.aborted) return;
        }
      }
      setDrafts(results);
      if (!draftOpen && results.length) {
        focusEditor.current = true;
        setDraftProject(results[0]?.project);
      }
      if (pairFromEditor) {
        setProjectOverride("both");
        if (replacedProject) setDraftProject(replacedProject);
        setRegenerateProject(undefined);
      } else if (replacement && succeeded) {
        const project = targets[0];
        setDraftProject(project);
        if (canChooseProject && !pairRequested)
          setProjectOverride(project ?? "");
        setRegenerateProject(undefined);
      }
      if (succeeded < targets.length)
        setFeedback(pairFromEditor && succeeded > 0 ? "generateFailed" : failure);
      else if (replacement)
        setFeedback(pairFromEditor ? "generatedPair" : "regenerated");
      else setFeedback(undefined);
    } catch {
      if (!run.signal.aborted) setFeedback(failure);
    } finally {
      operation.current = null;
      if (!run.signal.aborted) setBusy(undefined);
    }
  }

  function closeDraft() {
    returningToSelection.current = true;
    setDrafts([]);
    setRegenerateProject(undefined);
    setFeedback(undefined);
  }

  function restorePreviousDraft() {
    if (!ticket || !activeDraft?.previous || operation.current) return;
    const previous = activeDraft.previous;
    const next = drafts.map((entry) =>
      entry === activeDraft ? previous : entry,
    );
    runtime?.session.write(source.key, ticket.id, next);
    setDrafts(next);
    setDraftProject(previous.project);
    if (canChooseProject && !pairRequested)
      setProjectOverride(previous.project ?? "");
    setRegenerateProject(undefined);
    setFeedback("restoredDraft");
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
      (draftOpen && pairRequested)) && (
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
      {!analysisReady && (
        <p role="status">
          {source.analysisStatus === "failed"
            ? c.analysisFailed
            : c.analysisPending}
        </p>
      )}
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
          {canChooseProject && (
            <ProjectSkillSelect
              helpId={`${id}-project-help`}
              help={pairRequested ? c.combinedHint : c.projectUnknown}
              project={chosenProject}
              disabled={Boolean(busy)}
              onChange={setProjectOverride}
            />
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
                  : missing.length > 1
                    ? c.generateBoth
                    : cached.length
                      ? c.generateMissing
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
                      setRegenerateProject(undefined);
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
              readOnly={busy === "generate" || busy === "regenerate"}
              onChange={(event) => editDraft(event.currentTarget.value)}
            />
          </label>
          {canChooseRegeneration && (
            <ProjectSkillSelect
              helpId={`${id}-regenerate-project-help`}
              help={
                regenerationScope === "both" ? c.combinedHint : c.regenerateProjectHint
              }
              project={regenerationScope}
              disabled={!analysisReady || Boolean(busy)}
              onChange={setRegenerateProject}
            />
          )}
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
              ref={regenerateButton}
              type="button"
              className="secondary-button focus-visible-control"
              disabled={!ticket || !analysisReady || Boolean(busy)}
              onClick={() => {
                if (activeDraft)
                  void generate(true, {
                    current: activeDraft,
                    choice: regenerationScope,
                  });
              }}
            >
              <RefreshCw aria-hidden="true" />
              {busy === "regenerate" ? c.regenerating : regenerateLabel}
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
            <p
              role={
                feedback === "copyFailed" || feedback === "regenerateFailed"
                  ? "alert"
                  : "status"
              }
            >
              {c[feedback]}
            </p>
          )}
          {activeDraft?.previous && (
            <button
              type="button"
              className="secondary-button focus-visible-control"
              disabled={!ticket || Boolean(busy)}
              onClick={restorePreviousDraft}
            >
              {c.restoreDraft}
            </button>
          )}
          {retry}
        </>
      )}
    </section>
  );
}

function ProjectSkillSelect({
  project,
  helpId,
  help,
  disabled,
  onChange,
}: {
  project?: ProjectSkillChoice;
  helpId: string;
  help: string;
  disabled: boolean;
  onChange: (project: ProjectSkillChoice) => void;
}) {
  return (
    <label>
      <span>{c.project}</span>
      <select
        className="focus-visible-control"
        value={project ?? ""}
        disabled={disabled}
        aria-describedby={helpId}
        onChange={(event) => {
          const value = event.currentTarget.value;
          onChange(
            value === "both" || Object.hasOwn(ticketPromptProjects, value)
              ? (value as ProjectSkillChoice)
              : "",
          );
        }}
      >
        <option value="">{c.general}</option>
        {Object.entries(ticketPromptProjects).map(([value, info]) => (
          <option key={value} value={value}>
            {info.label}
          </option>
        ))}
        <option value="both">{c.bothProjects}</option>
      </select>
      <small id={helpId}>{help}</small>
    </label>
  );
}
