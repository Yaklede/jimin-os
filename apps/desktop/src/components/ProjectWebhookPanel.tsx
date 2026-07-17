import {
  History,
  PauseCircle,
  Pencil,
  PlayCircle,
  Plus,
  RotateCcw,
  Send,
  Trash2,
  Webhook,
} from "lucide-react";
import { FormEvent, useEffect, useRef, useState } from "react";

import {
  type ProjectWebhook,
  type ManagedWebhookProvider,
  type ProjectWebhookEvent,
  type WebhookDestinationMode,
  type WebhookDelivery,
} from "../api/webhooks";
import { copy } from "../copy";

const EVENT_OPTIONS: ProjectWebhookEvent[] = [
  "project.updated",
  "project.deleted",
  "task.created",
  "task.updated",
  "task.completed",
  "task.restored",
  "task.deleted",
];

type PendingWebhookAction = "create" | "edit" | "toggle" | "test" | "delete";

type ProjectWebhookPanelProps = {
  projectId: string;
  webhooks: ProjectWebhook[];
  deliveries: WebhookDelivery[];
  loading: boolean;
  saving: boolean;
  onCreate(input: {
    provider: ManagedWebhookProvider;
    url: string;
    events: ProjectWebhookEvent[];
  }): Promise<void>;
  onUpdate(
    webhook: ProjectWebhook,
    input: {
      provider: ManagedWebhookProvider;
      destinationMode: WebhookDestinationMode;
      url?: string;
      events: ProjectWebhookEvent[];
      enabled: boolean;
    },
  ): Promise<void>;
  onTest(webhook: ProjectWebhook): Promise<void>;
  onDelete(webhook: ProjectWebhook): Promise<void>;
  onRetry(delivery: WebhookDelivery): Promise<void>;
};

export function ProjectWebhookPanel({
  projectId,
  webhooks,
  deliveries,
  loading,
  saving,
  onCreate,
  onUpdate,
  onTest,
  onDelete,
  onRetry,
}: ProjectWebhookPanelProps) {
  const [formOpen, setFormOpen] = useState(false);
  const [provider, setProvider] =
    useState<ManagedWebhookProvider>("google_chat");
  const [url, setUrl] = useState("");
  const [events, setEvents] = useState<ProjectWebhookEvent[]>([
    "project.updated",
    "project.deleted",
    "task.created",
    "task.updated",
    "task.completed",
    "task.deleted",
  ]);
  const [deleteTarget, setDeleteTarget] = useState<string>();
  const [editTarget, setEditTarget] = useState<string>();
  const [retryTarget, setRetryTarget] = useState<string>();
  const [pendingAction, setPendingAction] = useState<{
    kind: PendingWebhookAction;
    id: string;
  }>();
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();
  const createTriggerRef = useRef<HTMLButtonElement>(null);
  const createUrlRef = useRef<HTMLInputElement>(null);
  const restoreCreateFocusRef = useRef(false);
  const editTriggerRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const restoreEditTargetRef = useRef<string | undefined>(undefined);
  const deleteTriggerRefs = useRef<Record<string, HTMLButtonElement | null>>(
    {},
  );
  const deleteSafeActionRef = useRef<HTMLButtonElement>(null);
  const restoreDeleteTargetRef = useRef<string | undefined>(undefined);
  const panelBusy = loading || saving || Boolean(pendingAction || retryTarget);

  useEffect(() => {
    setFormOpen(false);
    setDeleteTarget(undefined);
    setEditTarget(undefined);
    setRetryTarget(undefined);
    setPendingAction(undefined);
    restoreCreateFocusRef.current = false;
    restoreEditTargetRef.current = undefined;
    restoreDeleteTargetRef.current = undefined;
    setNotice(undefined);
    setError(undefined);
  }, [projectId]);

  useEffect(() => {
    const target = formOpen
      ? createUrlRef.current
      : restoreCreateFocusRef.current
        ? createTriggerRef.current
        : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!formOpen) restoreCreateFocusRef.current = false;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [formOpen]);

  useEffect(() => {
    if (editTarget || !restoreEditTargetRef.current) return;
    const targetId = restoreEditTargetRef.current;
    const frame = window.requestAnimationFrame(() => {
      editTriggerRefs.current[targetId]?.focus();
      restoreEditTargetRef.current = undefined;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [editTarget]);

  useEffect(() => {
    const restoreTarget = restoreDeleteTargetRef.current;
    const target = deleteTarget
      ? deleteSafeActionRef.current
      : restoreTarget
        ? deleteTriggerRefs.current[restoreTarget]
        : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!deleteTarget) restoreDeleteTargetRef.current = undefined;
    });
    return () => window.cancelAnimationFrame(frame);
  }, [deleteTarget]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (panelBusy) return;
    if (!url.trim() || events.length === 0) {
      setError(copy.projects.webhookRequired);
      return;
    }
    setError(undefined);
    setNotice(undefined);
    setPendingAction({ kind: "create", id: projectId });
    try {
      await onCreate({
        provider,
        url: url.trim(),
        events,
      });
      setUrl("");
      restoreCreateFocusRef.current = true;
      setFormOpen(false);
      setNotice(copy.projects.webhookSaved);
    } catch {
      setError(copy.projects.webhookSaveProblem);
    } finally {
      setPendingAction(undefined);
    }
  }

  function toggleEvent(value: ProjectWebhookEvent) {
    setEvents((current) =>
      current.includes(value)
        ? current.filter((event) => event !== value)
        : [...current, value],
    );
  }

  return (
    <section
      className="project-webhooks"
      aria-labelledby="webhook-title"
      aria-busy={panelBusy}
    >
      <div className="projects-section-heading project-webhooks__heading">
        <div>
          <Webhook aria-hidden="true" />
          <div>
            <h3 id="webhook-title">{copy.projects.webhookTitle}</h3>
            <p>{copy.projects.webhookDescription}</p>
          </div>
        </div>
        <button
          ref={createTriggerRef}
          className="secondary-button focus-visible-control"
          type="button"
          aria-expanded={formOpen}
          disabled={panelBusy}
          onClick={() => {
            setError(undefined);
            setNotice(undefined);
            setFormOpen((open) => {
              if (open) restoreCreateFocusRef.current = true;
              return !open;
            });
          }}
        >
          <Plus aria-hidden="true" />
          {copy.projects.webhookAdd}
        </button>
      </div>

      {(error || notice) && (
        <p
          className={error ? "inline-alert" : "project-save-status"}
          role={error ? "alert" : "status"}
        >
          {error || notice}
        </p>
      )}

      {formOpen && (
        <form
          className="project-webhook-form"
          aria-busy={pendingAction?.kind === "create"}
          onSubmit={(event) => void submit(event)}
        >
          <label htmlFor="project-webhook-provider">
            <span>{copy.projects.webhookProviderLabel}</span>
            <select
              id="project-webhook-provider"
              name="project-webhook-provider"
              value={provider}
              disabled={panelBusy}
              onChange={(event) => {
                setProvider(event.target.value as ManagedWebhookProvider);
                setUrl("");
              }}
            >
              <option value="google_chat">Google Chat</option>
              <option value="discord">Discord</option>
            </select>
          </label>
          <label htmlFor="project-webhook-url">
            <span>{copy.projects.webhookUrlLabel}</span>
            <input
              ref={createUrlRef}
              id="project-webhook-url"
              name="project-webhook-url"
              type="url"
              inputMode="url"
              autoComplete="url"
              required
              maxLength={4096}
              value={url}
              disabled={panelBusy}
              placeholder={copy.projects.webhookUrlHint(provider)}
              onChange={(event) => setUrl(event.target.value)}
            />
          </label>
          <fieldset>
            <legend>{copy.projects.webhookEventsLabel}</legend>
            <div className="project-webhook-events">
              {EVENT_OPTIONS.map((eventName) => (
                <label key={eventName}>
                  <input
                    id={`project-webhook-event-${eventName.replace(".", "-")}`}
                    type="checkbox"
                    name="project-webhook-events"
                    value={eventName}
                    checked={events.includes(eventName)}
                    disabled={panelBusy}
                    onChange={() => toggleEvent(eventName)}
                  />
                  <span>{copy.projects.webhookEvent(eventName)}</span>
                </label>
              ))}
            </div>
          </fieldset>
          <small>{copy.projects.webhookSecretDescription}</small>
          <div className="project-create-form__actions">
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={panelBusy}
              onClick={() => {
                restoreCreateFocusRef.current = true;
                setFormOpen(false);
              }}
            >
              {copy.actions.cancel}
            </button>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={panelBusy || !url.trim() || events.length === 0}
            >
              {pendingAction?.kind === "create" ? (
                <span className="button-spinner" aria-hidden="true" />
              ) : null}
              {pendingAction?.kind === "create"
                ? copy.actions.saving
                : copy.projects.webhookSave}
            </button>
          </div>
        </form>
      )}

      {loading ? (
        <p className="project-detail__empty" role="status">
          {copy.projects.webhookLoading}
        </p>
      ) : webhooks.length ? (
        <ul className="project-webhook-list">
          {webhooks.map((webhook) => (
            <li key={webhook.id}>
              <div className="project-webhook-list__main">
                <strong>
                  {copy.projects.webhookProvider(webhook.provider)}
                </strong>
                <span>{webhook.destinationLabel}</span>
                <span>
                  {webhook.events.map(copy.projects.webhookEvent).join(" · ")}
                </span>
                <small data-enabled={webhook.enabled}>
                  {webhook.enabled
                    ? copy.projects.webhookStatusActive
                    : copy.projects.webhookStatusPaused}
                </small>
                <small>
                  {webhook.provider === "legacy"
                    ? copy.projects.webhookLegacyNotice
                    : copy.projects.webhookSecretStored}
                </small>
              </div>
              <div className="project-webhook-list__actions">
                {webhook.provider !== "legacy" && (
                  <button
                    ref={(node) => {
                      editTriggerRefs.current[webhook.id] = node;
                    }}
                    className="secondary-button focus-visible-control"
                    type="button"
                    disabled={panelBusy}
                    aria-expanded={editTarget === webhook.id}
                    onClick={() => {
                      setDeleteTarget(undefined);
                      setEditTarget((current) => {
                        if (current === webhook.id) {
                          restoreEditTargetRef.current = webhook.id;
                          return undefined;
                        }
                        return webhook.id;
                      });
                    }}
                  >
                    <Pencil aria-hidden="true" />
                    {copy.projects.webhookEdit}
                  </button>
                )}
                {webhook.provider !== "legacy" && (
                  <button
                    className="secondary-button focus-visible-control"
                    type="button"
                    disabled={panelBusy}
                    onClick={async () => {
                      setPendingAction({ kind: "toggle", id: webhook.id });
                      setError(undefined);
                      setNotice(undefined);
                      try {
                        await onUpdate(webhook, {
                          provider:
                            webhook.provider as ManagedWebhookProvider,
                          destinationMode: "keep",
                          events: webhook.events,
                          enabled: !webhook.enabled,
                        });
                        setNotice(copy.projects.webhookUpdated);
                      } catch {
                        setError(copy.projects.webhookUpdateProblem);
                      } finally {
                        setPendingAction(undefined);
                      }
                    }}
                  >
                    {pendingAction?.kind === "toggle" &&
                    pendingAction.id === webhook.id ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : webhook.enabled ? (
                      <PauseCircle aria-hidden="true" />
                    ) : (
                      <PlayCircle aria-hidden="true" />
                    )}
                    {pendingAction?.kind === "toggle" &&
                    pendingAction.id === webhook.id
                      ? webhook.enabled
                        ? copy.projects.webhookPausing
                        : copy.projects.webhookResuming
                      : webhook.enabled
                        ? copy.projects.webhookPause
                        : copy.projects.webhookResume}
                  </button>
                )}
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={panelBusy}
                  onClick={async () => {
                    setPendingAction({ kind: "test", id: webhook.id });
                    setError(undefined);
                    setNotice(undefined);
                    try {
                      await onTest(webhook);
                      setNotice(copy.projects.webhookTestQueued);
                    } catch {
                      setError(copy.projects.webhookTestProblem);
                    } finally {
                      setPendingAction(undefined);
                    }
                  }}
                >
                  {pendingAction?.kind === "test" &&
                  pendingAction.id === webhook.id ? (
                    <span className="button-spinner" aria-hidden="true" />
                  ) : (
                    <Send aria-hidden="true" />
                  )}
                  {pendingAction?.kind === "test" &&
                  pendingAction.id === webhook.id
                    ? copy.projects.webhookTesting
                    : copy.projects.webhookTest}
                </button>
                <button
                  ref={(node) => {
                    deleteTriggerRefs.current[webhook.id] = node;
                  }}
                  className="destructive-quiet-button focus-visible-control"
                  type="button"
                  disabled={panelBusy}
                  aria-expanded={deleteTarget === webhook.id}
                  onClick={() => {
                    setEditTarget(undefined);
                    setDeleteTarget(webhook.id);
                  }}
                >
                  <Trash2 aria-hidden="true" />
                  {copy.projects.webhookDelete}
                </button>
              </div>
              {editTarget === webhook.id && webhook.provider !== "legacy" && (
                <WebhookEditForm
                  webhook={webhook}
                  saving={panelBusy}
                  onCancel={() => {
                    restoreEditTargetRef.current = webhook.id;
                    setEditTarget(undefined);
                  }}
                  onSave={async (input) => {
                    setPendingAction({ kind: "edit", id: webhook.id });
                    setError(undefined);
                    setNotice(undefined);
                    try {
                      await onUpdate(webhook, input);
                      restoreEditTargetRef.current = webhook.id;
                      setEditTarget(undefined);
                      setNotice(copy.projects.webhookUpdated);
                    } catch {
                      setError(copy.projects.webhookUpdateProblem);
                    } finally {
                      setPendingAction(undefined);
                    }
                  }}
                />
              )}
              {deleteTarget === webhook.id && (
                <div
                  className="project-webhook-list__confirm"
                  role="group"
                  aria-label={copy.projects.webhookDeleteConfirm}
                >
                  <p>{copy.projects.webhookDeleteConfirm}</p>
                  <div>
                    <button
                      ref={deleteSafeActionRef}
                      className="secondary-button focus-visible-control"
                      type="button"
                      disabled={panelBusy}
                      onClick={() => {
                        restoreDeleteTargetRef.current = webhook.id;
                        setDeleteTarget(undefined);
                      }}
                    >
                      {copy.projects.webhookKeep}
                    </button>
                    <button
                      className="destructive-button focus-visible-control"
                      type="button"
                      disabled={panelBusy}
                      onClick={async () => {
                        setPendingAction({ kind: "delete", id: webhook.id });
                        setError(undefined);
                        try {
                          await onDelete(webhook);
                          setDeleteTarget(undefined);
                          setNotice(copy.projects.webhookDeleted);
                        } catch {
                          setError(copy.projects.webhookDeleteProblem);
                          restoreDeleteTargetRef.current = webhook.id;
                          setDeleteTarget(undefined);
                        } finally {
                          setPendingAction(undefined);
                        }
                      }}
                    >
                      {pendingAction?.kind === "delete" &&
                      pendingAction.id === webhook.id ? (
                        <span className="button-spinner" aria-hidden="true" />
                      ) : (
                        <Trash2 aria-hidden="true" />
                      )}
                      {pendingAction?.kind === "delete" &&
                      pendingAction.id === webhook.id
                        ? copy.projects.webhookDeleting
                        : copy.projects.webhookDeleteAction}
                    </button>
                  </div>
                </div>
              )}
            </li>
          ))}
        </ul>
      ) : (
        <p className="project-detail__empty">{copy.projects.webhookEmpty}</p>
      )}

      {deliveries.length > 0 && (
        <section
          className="project-webhook-history"
          aria-labelledby="webhook-history-title"
        >
          <div>
            <History aria-hidden="true" />
            <h4 id="webhook-history-title">
              {copy.projects.webhookHistoryTitle}
            </h4>
          </div>
          <ul>
            {deliveries.slice(0, 10).map((delivery) => (
              <li key={delivery.id}>
                <span data-status={delivery.status}>
                  {copy.projects.webhookDeliveryStatus(delivery.status)}
                </span>
                <strong>
                  {copy.projects.webhookEvent(delivery.eventType)}
                </strong>
                <time dateTime={delivery.createdAt}>
                  {formatDeliveryTime(delivery.createdAt)}
                </time>
                <small>
                  {copy.projects.webhookDeliveryMeta(
                    delivery.attemptCount,
                    delivery.responseCode ?? undefined,
                  )}
                </small>
                {delivery.status === "failed" && (
                  <button
                    className="text-button focus-visible-control project-webhook-history__retry"
                    type="button"
                    disabled={panelBusy}
                    onClick={async () => {
                      setRetryTarget(delivery.id);
                      setError(undefined);
                      setNotice(undefined);
                      try {
                        await onRetry(delivery);
                        setNotice(copy.projects.webhookRetryQueued);
                      } catch {
                        setError(copy.projects.webhookRetryProblem);
                      } finally {
                        setRetryTarget(undefined);
                      }
                    }}
                  >
                    {retryTarget === delivery.id ? (
                      <span className="button-spinner" aria-hidden="true" />
                    ) : (
                      <RotateCcw aria-hidden="true" />
                    )}
                    {retryTarget === delivery.id
                      ? copy.projects.webhookRetrying
                      : copy.projects.webhookRetry}
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}
    </section>
  );
}

function WebhookEditForm({
  webhook,
  saving,
  onCancel,
  onSave,
}: {
  webhook: ProjectWebhook;
  saving: boolean;
  onCancel(): void;
  onSave(input: {
    provider: ManagedWebhookProvider;
    destinationMode: WebhookDestinationMode;
    url?: string;
    events: ProjectWebhookEvent[];
    enabled: boolean;
  }): Promise<void>;
}) {
  const provider = webhook.provider as ManagedWebhookProvider;
  const [destinationMode, setDestinationMode] =
    useState<WebhookDestinationMode>("keep");
  const [url, setUrl] = useState("");
  const [events, setEvents] = useState<ProjectWebhookEvent[]>(webhook.events);
  const [enabled, setEnabled] = useState(webhook.enabled);
  const [validation, setValidation] = useState<string>();
  const id = `webhook-edit-${webhook.id}`;

  function toggleEvent(value: ProjectWebhookEvent) {
    setEvents((current) =>
      current.includes(value)
        ? current.filter((event) => event !== value)
        : [...current, value],
    );
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (events.length === 0 || (destinationMode === "replace" && !url.trim())) {
      setValidation(copy.projects.webhookRequired);
      return;
    }
    setValidation(undefined);
    await onSave({
      provider,
      destinationMode,
      url: destinationMode === "replace" ? url.trim() : undefined,
      events,
      enabled,
    });
  }

  return (
    <form
      className="project-webhook-form project-webhook-form--edit"
      aria-labelledby={`${id}-title`}
      aria-busy={saving}
      onSubmit={(event) => void submit(event)}
    >
      <div className="project-webhook-form__heading">
        <div>
          <h4 id={`${id}-title`}>{copy.projects.webhookEditTitle}</h4>
          <p>{copy.projects.webhookEditDescription}</p>
        </div>
      </div>
      <label htmlFor={`${id}-destination-mode`}>
        <span>{copy.projects.webhookDestinationModeLabel}</span>
        <select
          id={`${id}-destination-mode`}
          name={`${id}-destination-mode`}
          value={destinationMode}
          disabled={saving}
          onChange={(event) => {
            setDestinationMode(event.target.value as WebhookDestinationMode);
            setUrl("");
            setValidation(undefined);
          }}
        >
          <option value="keep">{copy.projects.webhookDestinationKeep}</option>
          <option value="replace">
            {copy.projects.webhookDestinationReplace}
          </option>
        </select>
      </label>
      {destinationMode === "replace" && (
        <label htmlFor={`${id}-url`}>
          <span>{copy.projects.webhookUrlLabel}</span>
          <input
            id={`${id}-url`}
            name={`${id}-url`}
            type="url"
            inputMode="url"
            autoComplete="url"
            autoFocus
            required
            maxLength={4096}
            value={url}
            disabled={saving}
            placeholder={copy.projects.webhookUrlHint(provider)}
            onChange={(event) => {
              setUrl(event.target.value);
              setValidation(undefined);
            }}
          />
          <small>{copy.projects.webhookSecretDescription}</small>
        </label>
      )}
      <fieldset>
        <legend>{copy.projects.webhookEventsLabel}</legend>
        <div className="project-webhook-events">
          {EVENT_OPTIONS.map((eventName) => (
            <label key={eventName}>
              <input
                id={`${id}-event-${eventName.replace(".", "-")}`}
                name={`${id}-events`}
                type="checkbox"
                value={eventName}
                checked={events.includes(eventName)}
                disabled={saving}
                onChange={() => toggleEvent(eventName)}
              />
              <span>{copy.projects.webhookEvent(eventName)}</span>
            </label>
          ))}
        </div>
      </fieldset>
      <label
        className="project-webhook-form__enabled"
        htmlFor={`${id}-enabled`}
      >
        <input
          id={`${id}-enabled`}
          name={`${id}-enabled`}
          type="checkbox"
          checked={enabled}
          disabled={saving}
          onChange={(event) => setEnabled(event.target.checked)}
        />
        <span>{copy.projects.webhookEnabledLabel}</span>
      </label>
      {validation && (
        <p className="inline-alert" role="alert">
          {validation}
        </p>
      )}
      <div className="project-create-form__actions">
        <button
          className="secondary-button focus-visible-control"
          type="button"
          disabled={saving}
          onClick={onCancel}
        >
          {copy.projects.webhookStopEditing}
        </button>
        <button
          className="primary-button focus-visible-control"
          type="submit"
          disabled={
            saving ||
            events.length === 0 ||
            (destinationMode === "replace" && !url.trim())
          }
        >
          {saving ? copy.actions.saving : copy.projects.webhookSaveChanges}
        </button>
      </div>
    </form>
  );
}

function formatDeliveryTime(value: string): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(value));
}
