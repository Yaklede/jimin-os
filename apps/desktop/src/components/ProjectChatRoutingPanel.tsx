import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { RefreshCw } from "lucide-react";
import {
  deleteTaskChatRoute,
  fetchTaskChatRoute,
  saveTaskChatRoute,
  type ChatAuthenticate,
  type TaskChatRoute,
} from "../api/chatRouting";
import { PlanningRequestError } from "../api/planning";
import type { ProjectGoogleChatSource } from "../api/googleChat";
import type { ProjectWebhook } from "../api/webhooks";
import { chatRoutingCopy as c } from "../copy/chatRouting";
import "./chatRouting.css";

type Props = {
  baseUrl: string;
  authenticate: ChatAuthenticate;
  projectId: string;
  sources: ProjectGoogleChatSource[];
  webhooks: ProjectWebhook[];
};
export function ProjectChatRoutingPanel({
  baseUrl,
  authenticate,
  projectId,
  sources,
  webhooks,
}: Props) {
  const [route, setRoute] = useState<TaskChatRoute | null>(null);
  const [sourceId, setSourceId] = useState("");
  const [directory, setDirectory] = useState("");
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [removing, setRemoving] = useState(false);
  const generation = useRef(0);
  const pending = useRef(false);
  const removeButton = useRef<HTMLButtonElement>(null);
  const keepButton = useRef<HTMLButtonElement>(null);
  const roomSelect = useRef<HTMLSelectElement>(null);
  const load = useCallback(async () => {
    const epoch = ++generation.current;
    setLoading(true);
    setLoaded(false);
    setError("");
    setNotice("");
    setRemoving(false);
    setBusy(false);
    pending.current = false;
    try {
      const result = await authenticate((access) =>
        fetchTaskChatRoute(baseUrl, access, projectId),
      );
      if (epoch !== generation.current) return;
      setRoute(result.item);
      setSourceId(result.item?.sourceId ?? "");
      setDirectory(result.item?.directoryWebhookId ?? "");
      setLoaded(true);
    } catch {
      if (epoch === generation.current) setError(c.loadError);
    } finally {
      if (epoch === generation.current) setLoading(false);
    }
  }, [authenticate, baseUrl, projectId]);
  useEffect(() => {
    void load();
    return () => {
      ++generation.current;
    };
  }, [load]);
  useEffect(() => {
    if (removing) keepButton.current?.focus();
  }, [removing]);
  useEffect(() => {
    if (notice === c.removed && !busy) roomSelect.current?.focus();
  }, [notice, busy]);
  const rooms = sources.filter(
    (s) => s.enabled && (s.purpose === "delivery" || s.purpose === "both"),
  );
  const directories = webhooks.filter((h) => h.provider === "google_chat");
  const valid =
    rooms.some((s) => s.id === sourceId) &&
    directories.some((h) => h.id === directory);
  const changed =
    sourceId !== route?.sourceId || directory !== route?.directoryWebhookId;
  async function save(event: FormEvent) {
    event.preventDefault();
    if (pending.current || !valid || !loaded) return;
    pending.current = true;
    setBusy(true);
    setError("");
    setNotice("");
    const epoch = generation.current;
    try {
      const result = await authenticate((access) =>
        saveTaskChatRoute(baseUrl, access, projectId, {
          sourceId,
          directoryWebhookId: directory,
          expectedVersion: route?.version ?? 0,
        }),
      );
      if (epoch === generation.current) {
        setRoute(result.item);
        setNotice(c.saved);
      }
    } catch (failure) {
      if (epoch === generation.current)
        setError(
          failure instanceof PlanningRequestError && failure.code === "conflict"
            ? c.conflict
            : c.saveError,
        );
    } finally {
      if (epoch === generation.current) {
        pending.current = false;
        setBusy(false);
      }
    }
  }
  async function remove() {
    if (pending.current || !route) return;
    pending.current = true;
    setBusy(true);
    setError("");
    setNotice("");
    const epoch = generation.current;
    try {
      await authenticate((access) =>
        deleteTaskChatRoute(baseUrl, access, route),
      );
      if (epoch === generation.current) {
        setRoute(null);
        setSourceId("");
        setDirectory("");
        setRemoving(false);
        setNotice(c.removed);
      }
    } catch (failure) {
      if (epoch === generation.current)
        setError(
          failure instanceof PlanningRequestError && failure.code === "conflict"
            ? c.conflict
            : c.saveError,
        );
    } finally {
      if (epoch === generation.current) {
        pending.current = false;
        setBusy(false);
      }
    }
  }
  function cancelRemove() {
    setRemoving(false);
    removeButton.current?.focus();
  }
  return (
    <section
      className="project-chat-routing"
      aria-label={c.title}
      aria-busy={loading || busy}
    >
      <header>
        <div>
          <h4>{c.title}</h4>
          <p>{c.help}</p>
        </div>
        <button
          type="button"
          className="icon-button focus-visible-control"
          disabled={loading || busy}
          onClick={() => void load()}
          aria-label={c.load}
        >
          <RefreshCw aria-hidden="true" />
        </button>
      </header>
      <div className="project-chat-routing__load-status" role="status">
        {loading ? c.loading : ""}
      </div>
      <>
        {!loading && !rooms.length && <p>{c.empty}</p>}
        <form onSubmit={(event) => void save(event)}>
          <label>
            <span>{c.room}</span>
            <select
              ref={roomSelect}
              aria-label={c.room}
              value={sourceId}
              disabled={busy || !loaded}
              required
              onChange={(event) => setSourceId(event.target.value)}
            >
              <option value="">{c.chooseRoom}</option>
              {!!sourceId && !rooms.some((s) => s.id === sourceId) && (
                <option value={sourceId} disabled>
                  {c.missing}
                </option>
              )}
              {rooms.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.displayName} ({s.accountEmail})
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>{c.directory}</span>
            <select
              aria-label={c.directory}
              value={directory}
              disabled={busy || !loaded}
              required
              onChange={(event) => setDirectory(event.target.value)}
            >
              <option value="">{c.chooseDirectory}</option>
              {!!directory && !directories.some((h) => h.id === directory) && (
                <option value={directory} disabled>
                  {c.missing}
                </option>
              )}
              {directories.map((h) => (
                <option key={h.id} value={h.id}>
                  {h.destinationLabel} ·{" "}
                  {Object.keys(h.mentionDirectory.users).length}명
                </option>
              ))}
            </select>
          </label>
          <p className="project-chat-routing__help">{c.directoryHelp}</p>
          <p className="project-chat-routing__help">{c.replyHelp}</p>
          <p className="project-chat-routing__help">{c.duplicateHelp}</p>
          <div className="project-chat-routing__actions">
            <button
              type="submit"
              className="primary-button focus-visible-control"
              disabled={busy || !loaded || !valid || !changed}
            >
              {busy ? c.saving : c.save}
            </button>
            {route && (
              <button
                ref={removeButton}
                type="button"
                className="secondary-button focus-visible-control"
                disabled={busy}
                onClick={() => setRemoving(true)}
              >
                {c.remove}
              </button>
            )}
          </div>
        </form>
        {removing && (
          <div
            className="inline-alert"
            role="group"
            aria-label={c.remove}
            onKeyDown={(event) => {
              if (event.key === "Escape" && !busy) {
                event.stopPropagation();
                cancelRemove();
              }
            }}
          >
            <p>{c.removeQuestion}</p>
            <div className="project-chat-routing__actions">
              <button
                ref={keepButton}
                type="button"
                className="secondary-button focus-visible-control"
                disabled={busy}
                onClick={cancelRemove}
              >
                {c.keep}
              </button>
              <button
                type="button"
                className="secondary-button focus-visible-control"
                disabled={busy}
                onClick={() => void remove()}
              >
                {c.confirmRemove}
              </button>
            </div>
          </div>
        )}
      </>
      {error && (
        <p className="inline-alert" role="alert">
          {error}
        </p>
      )}
      {notice && <p role="status">{notice}</p>}
    </section>
  );
}
