import {
  Check,
  Eye,
  Inbox,
  Link2,
  LoaderCircle,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";
import { FormEvent, useEffect, useMemo, useState } from "react";

import {
  type GoogleChatAccount,
  type GoogleChatSpace,
  type ProjectGoogleChatSource,
  type ProjectInflowItem,
} from "../api/googleChat";
import { copy } from "../copy";

type ProjectInflowPanelProps = {
  accountsAvailable: boolean;
  accounts: GoogleChatAccount[];
  spaces: GoogleChatSpace[];
  sources: ProjectGoogleChatSource[];
  items: ProjectInflowItem[];
  loading: boolean;
  saving: boolean;
  problemMessage?: string;
  onConnectAccount(): Promise<void>;
  onLoadSpaces(accountId: string): Promise<void>;
  onCreateSource(input: {
    accountId: string;
    spaceName: string;
    displayName: string;
    acknowledgeWithReaction: boolean;
  }): Promise<void>;
  onDeleteSource(source: ProjectGoogleChatSource): Promise<void>;
  onSyncSource(source: ProjectGoogleChatSource): Promise<void>;
  onPromote(item: ProjectInflowItem, title: string): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
};

export function ProjectInflowPanel({
  accountsAvailable,
  accounts,
  spaces,
  sources,
  items,
  loading,
  saving,
  problemMessage,
  onConnectAccount,
  onLoadSpaces,
  onCreateSource,
  onDeleteSource,
  onSyncSource,
  onPromote,
  onDismiss,
}: ProjectInflowPanelProps) {
  const activeAccounts = useMemo(
    () => accounts.filter((account) => account.status === "active"),
    [accounts],
  );
  const [accountId, setAccountId] = useState("");
  const [spaceName, setSpaceName] = useState("");
  const [acknowledge, setAcknowledge] = useState(true);

  useEffect(() => {
    const next = activeAccounts.some((account) => account.id === accountId)
      ? accountId
      : (activeAccounts[0]?.id ?? "");
    if (next !== accountId) setAccountId(next);
    if (next) void onLoadSpaces(next);
  }, [accountId, activeAccounts, onLoadSpaces]);

  useEffect(() => {
    if (!spaces.some((space) => space.name === spaceName)) setSpaceName("");
  }, [spaceName, spaces]);

  async function addSource(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const space = spaces.find((item) => item.name === spaceName);
    if (!accountId || !space) return;
    await onCreateSource({
      accountId,
      spaceName: space.name,
      displayName: space.displayName,
      acknowledgeWithReaction: acknowledge,
    });
    setSpaceName("");
  }

  return (
    <section className="project-inflow" aria-labelledby="project-inflow-title">
      <header className="project-inflow__heading">
        <div className="project-inflow__heading-icon" aria-hidden="true">
          <Inbox />
        </div>
        <div>
          <h3 id="project-inflow-title">{copy.projects.inflowTitle}</h3>
          <p>{copy.projects.inflowDescription}</p>
        </div>
      </header>

      {problemMessage && (
        <p className="inline-alert" role="alert">
          {problemMessage}
        </p>
      )}

      {activeAccounts.length === 0 ? (
        <div className="project-inflow__connect">
          <div>
            <strong>{copy.projects.inflowConnectAccount}</strong>
            <p>{copy.projects.inflowConnectDescription}</p>
          </div>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={!accountsAvailable || saving}
            onClick={() => void onConnectAccount()}
          >
            <Link2 aria-hidden="true" />
            {copy.projects.inflowConnectAccount}
          </button>
        </div>
      ) : (
        <form
          className="project-inflow__source-form"
          onSubmit={(event) => void addSource(event)}
        >
          <label>
            <span>{copy.projects.inflowAccountLabel}</span>
            <select
              value={accountId}
              disabled={saving}
              onChange={(event) => {
                setAccountId(event.target.value);
                setSpaceName("");
              }}
            >
              {activeAccounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {account.email}
                </option>
              ))}
            </select>
          </label>
          <label>
            <span>{copy.projects.inflowSpaceLabel}</span>
            <select
              value={spaceName}
              disabled={loading || saving}
              onChange={(event) => setSpaceName(event.target.value)}
            >
              <option value="">{copy.projects.inflowChooseSpace}</option>
              {spaces.map((space) => (
                <option key={space.name} value={space.name}>
                  {space.displayName}
                </option>
              ))}
            </select>
          </label>
          <div className="project-inflow__source-form-actions">
            <label className="project-inflow__acknowledge">
              <input
                type="checkbox"
                checked={acknowledge}
                disabled={saving}
                onChange={(event) => setAcknowledge(event.target.checked)}
              />
              <span>{copy.projects.inflowAckLabel}</span>
            </label>
            <button
              className="secondary-button focus-visible-control"
              type="submit"
              disabled={!spaceName || saving}
            >
              {saving ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : (
                <Link2 aria-hidden="true" />
              )}
              {copy.projects.inflowAddSource}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={!accountsAvailable || saving}
              onClick={() => void onConnectAccount()}
            >
              <Link2 aria-hidden="true" />
              {copy.projects.inflowConnectAnotherAccount}
            </button>
          </div>
        </form>
      )}

      {sources.length > 0 && (
        <ul className="project-inflow__sources" aria-label="연결된 Chat 공간">
          {sources.map((source) => (
            <li key={source.id}>
              <div>
                <strong>{source.displayName}</strong>
                <span>{source.accountEmail}</span>
              </div>
              <div className="project-inflow__source-actions">
                {source.acknowledgeWithReaction && (
                  <span className="project-inflow__ack-state">
                    <Eye aria-hidden="true" /> 확인 표시
                  </span>
                )}
                <button
                  className="icon-button focus-visible-control"
                  type="button"
                  aria-label={`${source.displayName} ${copy.projects.inflowRefresh}`}
                  disabled={loading || saving}
                  onClick={() => void onSyncSource(source)}
                >
                  <RefreshCw aria-hidden="true" />
                </button>
                <button
                  className="icon-button focus-visible-control"
                  type="button"
                  aria-label={`${source.displayName} ${copy.projects.inflowRemoveSource}`}
                  disabled={saving}
                  onClick={() => void onDeleteSource(source)}
                >
                  <Trash2 aria-hidden="true" />
                </button>
              </div>
            </li>
          ))}
        </ul>
      )}
      {sources.length === 0 && activeAccounts.length > 0 && (
        <p className="project-inflow__empty">{copy.projects.inflowNoSource}</p>
      )}

      {sources.length > 0 && (
        <div className="project-inflow__items" aria-busy={loading}>
          {loading && items.length === 0 ? (
            <p className="project-inflow__empty">
              <LoaderCircle className="spin" aria-hidden="true" /> 새 메시지를
              확인하고 있어요.
            </p>
          ) : items.length === 0 ? (
            <p className="project-inflow__empty">{copy.projects.inflowEmpty}</p>
          ) : (
            <ul>
              {items.map((item) => (
                <InflowItemRow
                  key={item.id}
                  item={item}
                  saving={saving}
                  onPromote={onPromote}
                  onDismiss={onDismiss}
                />
              ))}
            </ul>
          )}
        </div>
      )}
    </section>
  );
}

function InflowItemRow({
  item,
  saving,
  onPromote,
  onDismiss,
}: {
  item: ProjectInflowItem;
  saving: boolean;
  onPromote(item: ProjectInflowItem, title: string): Promise<void>;
  onDismiss(item: ProjectInflowItem): Promise<void>;
}) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(() =>
    suggestedTaskTitle(item.contentText),
  );

  return (
    <li className="project-inflow-item">
      <div className="project-inflow-item__meta">
        <span>{item.sourceName}</span>
        <span>{item.senderName ?? "보낸 사람 정보 없음"}</span>
        <time dateTime={item.receivedAt}>
          {formatReceivedAt(item.receivedAt)}
        </time>
        {item.acknowledged && <span>👀 표시 완료</span>}
      </div>
      <p>{item.contentText}</p>
      {editing ? (
        <form
          className="project-inflow-item__promote"
          onSubmit={(event) => {
            event.preventDefault();
            if (title.trim()) void onPromote(item, title.trim());
          }}
        >
          <label>
            <span>{copy.projects.inflowTaskTitleLabel}</span>
            <input
              value={title}
              maxLength={300}
              disabled={saving}
              aria-describedby={`inflow-task-title-help-${item.id}`}
              onChange={(event) => setTitle(event.target.value)}
            />
            <small id={`inflow-task-title-help-${item.id}`}>
              {copy.projects.inflowTaskTitleHint}
            </small>
          </label>
          <div>
            <button
              className="primary-button focus-visible-control"
              type="submit"
              disabled={!title.trim() || saving}
            >
              <Check aria-hidden="true" /> {copy.projects.inflowPromote}
            </button>
            <button
              className="secondary-button focus-visible-control"
              type="button"
              disabled={saving}
              onClick={() => setEditing(false)}
            >
              <X aria-hidden="true" /> 취소
            </button>
          </div>
        </form>
      ) : (
        <div className="project-inflow-item__actions">
          <button
            className="primary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => setEditing(true)}
          >
            <Check aria-hidden="true" /> {copy.projects.inflowPromote}
          </button>
          <button
            className="secondary-button focus-visible-control"
            type="button"
            disabled={saving}
            onClick={() => void onDismiss(item)}
          >
            <X aria-hidden="true" /> {copy.projects.inflowDismiss}
          </button>
        </div>
      )}
    </li>
  );
}

function suggestedTaskTitle(content: string): string {
  const firstLine = content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean);
  return (firstLine ?? "확인할 요청").replace(/^[\-*•]+\s*/, "").slice(0, 80);
}

function formatReceivedAt(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return "받은 시간 확인 필요";
  return new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}
