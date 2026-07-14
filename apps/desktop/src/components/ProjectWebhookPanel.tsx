import { History, Plus, Send, Trash2, Webhook } from "lucide-react";
import { FormEvent, useEffect, useState } from "react";

import {
  type ProjectWebhook,
  type ProjectWebhookEvent,
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

type ProjectWebhookPanelProps = {
  projectId: string;
  webhooks: ProjectWebhook[];
  deliveries: WebhookDelivery[];
  loading: boolean;
  saving: boolean;
  onCreate(input: {
    url: string;
    events: ProjectWebhookEvent[];
    authorization?: string;
  }): Promise<void>;
  onTest(webhook: ProjectWebhook): Promise<void>;
  onDelete(webhook: ProjectWebhook): Promise<void>;
};

export function ProjectWebhookPanel({
  projectId,
  webhooks,
  deliveries,
  loading,
  saving,
  onCreate,
  onTest,
  onDelete,
}: ProjectWebhookPanelProps) {
  const [formOpen, setFormOpen] = useState(false);
  const [url, setUrl] = useState("");
  const [authorization, setAuthorization] = useState("");
  const [events, setEvents] = useState<ProjectWebhookEvent[]>([
    "project.updated",
    "project.deleted",
    "task.created",
    "task.updated",
    "task.completed",
    "task.deleted",
  ]);
  const [deleteTarget, setDeleteTarget] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [error, setError] = useState<string>();

  useEffect(() => {
    setFormOpen(false);
    setDeleteTarget(undefined);
    setNotice(undefined);
    setError(undefined);
  }, [projectId]);

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!url.trim() || events.length === 0) {
      setError(copy.projects.webhookRequired);
      return;
    }
    setError(undefined);
    setNotice(undefined);
    try {
      await onCreate({
        url: url.trim(),
        events,
        authorization: authorization.trim() || undefined,
      });
      setUrl("");
      setAuthorization("");
      setFormOpen(false);
      setNotice(copy.projects.webhookSaved);
    } catch {
      setError(copy.projects.webhookSaveProblem);
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
    <section className="project-webhooks" aria-labelledby="webhook-title">
      <div className="projects-section-heading project-webhooks__heading">
        <div>
          <Webhook aria-hidden="true" />
          <div>
            <h3 id="webhook-title">{copy.projects.webhookTitle}</h3>
            <p>{copy.projects.webhookDescription}</p>
          </div>
        </div>
        <button
          className="secondary-button focus-visible-control"
          type="button"
          aria-expanded={formOpen}
          disabled={saving}
          onClick={() => {
            setError(undefined);
            setNotice(undefined);
            setFormOpen((open) => !open);
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
          onSubmit={(event) => void submit(event)}
        >
          <label htmlFor="project-webhook-url">
            <span>{copy.projects.webhookUrlLabel}</span>
            <input
              id="project-webhook-url"
              type="url"
              inputMode="url"
              autoComplete="url"
              autoFocus
              required
              maxLength={4096}
              value={url}
              disabled={saving}
              placeholder={copy.projects.webhookUrlHint}
              onChange={(event) => setUrl(event.target.value)}
            />
          </label>
          <fieldset>
            <legend>{copy.projects.webhookEventsLabel}</legend>
            <div className="project-webhook-events">
              {EVENT_OPTIONS.map((eventName) => (
                <label key={eventName}>
                  <input
                    type="checkbox"
                    name="project-webhook-events"
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
          <label htmlFor="project-webhook-authorization">
            <span>{copy.projects.webhookAuthorizationLabel}</span>
            <input
              id="project-webhook-authorization"
              type="password"
              autoComplete="new-password"
              maxLength={8192}
              value={authorization}
              disabled={saving}
              placeholder={copy.projects.webhookAuthorizationHint}
              onChange={(event) => setAuthorization(event.target.value)}
            />
            <small>{copy.projects.webhookAuthorizationDescription}</small>
          </label>
          <div className="project-create-form__actions">
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => setFormOpen(false)}
            >
              {copy.actions.cancel}
            </button>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={saving || !url.trim() || events.length === 0}
            >
              {saving ? copy.actions.saving : copy.projects.webhookSave}
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
                <strong>{webhook.url}</strong>
                <span>
                  {webhook.events.map(copy.projects.webhookEvent).join(" · ")}
                </span>
                {webhook.hasAuthentication && (
                  <small>{copy.projects.webhookAuthenticationStored}</small>
                )}
              </div>
              <div className="project-webhook-list__actions">
                <button
                  className="secondary-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  onClick={async () => {
                    setError(undefined);
                    setNotice(undefined);
                    try {
                      await onTest(webhook);
                      setNotice(copy.projects.webhookTestQueued);
                    } catch {
                      setError(copy.projects.webhookTestProblem);
                    }
                  }}
                >
                  <Send aria-hidden="true" />
                  {copy.projects.webhookTest}
                </button>
                <button
                  className="destructive-quiet-button focus-visible-control"
                  type="button"
                  disabled={saving}
                  aria-expanded={deleteTarget === webhook.id}
                  onClick={() => setDeleteTarget(webhook.id)}
                >
                  <Trash2 aria-hidden="true" />
                  {copy.projects.webhookDelete}
                </button>
              </div>
              {deleteTarget === webhook.id && (
                <div className="project-webhook-list__confirm" role="alert">
                  <p>{copy.projects.webhookDeleteConfirm}</p>
                  <div>
                    <button
                      className="secondary-button focus-visible-control"
                      type="button"
                      disabled={saving}
                      onClick={() => setDeleteTarget(undefined)}
                    >
                      {copy.projects.webhookKeep}
                    </button>
                    <button
                      className="destructive-button focus-visible-control"
                      type="button"
                      disabled={saving}
                      onClick={async () => {
                        setError(undefined);
                        try {
                          await onDelete(webhook);
                          setDeleteTarget(undefined);
                          setNotice(copy.projects.webhookDeleted);
                        } catch {
                          setError(copy.projects.webhookDeleteProblem);
                        }
                      }}
                    >
                      {copy.projects.webhookDeleteAction}
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
              </li>
            ))}
          </ul>
        </section>
      )}
    </section>
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
