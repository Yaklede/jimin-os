import { useRef, useState } from "react";
import { ChevronDown, RefreshCw } from "lucide-react";
import type { ProjectInflowItem } from "../api/googleChat";
import { inflowRecoveryCopy as text } from "../copy/inflowRecovery";
import {
  canRetryInflowDelivery,
  inflowDeliveryHelp,
  inflowRecoveryItems,
} from "../inflowDeliveryState";
import { InflowDeliveryStatus } from "./InflowDeliveryStatus";

export function InflowRecoveryPanel({
  items,
  saving,
  onRetry,
  onOpenTask,
}: {
  items: ProjectInflowItem[];
  saving: boolean;
  onRetry(item: ProjectInflowItem): Promise<void>;
  onOpenTask?(id: string): Promise<void>;
}) {
  const recovery = inflowRecoveryItems(items);
  const [busyId, setBusyId] = useState<string>();
  const [notice, setNotice] = useState<{ id: string; message: string }>();
  const summaryRef = useRef<HTMLElement>(null);
  if (!recovery.length) return null;
  const failures = recovery.filter(canRetryInflowDelivery).length;
  async function run(
    item: ProjectInflowItem,
    action: () => Promise<void>,
    problem: string,
  ) {
    if (busyId || saving) return;
    setBusyId(item.id);
    setNotice(undefined);
    try {
      await action();
      summaryRef.current?.focus();
    } catch {
      setNotice({ id: item.id, message: problem });
    } finally {
      setBusyId(undefined);
    }
  }
  return (
    <details className="inflow-recovery">
      <summary ref={summaryRef} className="focus-visible-control">
        <span>
          <strong>{text.title}</strong>
          <small>{text.count(failures, recovery.length - failures)}</small>
        </span>
        <ChevronDown aria-hidden="true" />
      </summary>
      <div className="inflow-recovery__body">
        <p>{text.description}</p>
        <ul>
          {recovery.map((item) => (
            <li
              key={`${item.projectId}:${item.id}`}
              aria-busy={busyId === item.id}
            >
              <div className="inflow-recovery__heading">
                <small>
                  {item.projectName} · {item.sourceName}
                </small>
                <h3>{item.suggestedTaskTitle}</h3>
              </div>
              <InflowDeliveryStatus item={item} />
              <p>{inflowDeliveryHelp(item)}</p>
              <div className="inflow-recovery__actions">
                {canRetryInflowDelivery(item) && (
                  <button
                    type="button"
                    className="secondary-button focus-visible-control"
                    disabled={saving || Boolean(busyId)}
                    aria-label={`${item.suggestedTaskTitle}: ${item.status === "dismissed" ? text.retryDismissal : text.retry}`}
                    onClick={() =>
                      void run(item, () => onRetry(item), text.requestFailed)
                    }
                  >
                    <RefreshCw aria-hidden="true" />
                    {busyId === item.id
                      ? text.retrying
                      : item.status === "dismissed"
                        ? text.retryDismissal
                        : text.retry}
                  </button>
                )}
                {item.promotedTaskId && onOpenTask && (
                  <button
                    type="button"
                    className="text-button focus-visible-control"
                    disabled={saving || Boolean(busyId)}
                    aria-label={`${item.suggestedTaskTitle}: ${text.openTask}`}
                    onClick={() =>
                      void run(
                        item,
                        () => onOpenTask(item.promotedTaskId!),
                        text.openFailed,
                      )
                    }
                  >
                    {text.openTask}
                  </button>
                )}
              </div>
              {notice?.id === item.id && (
                <p className="inline-alert" role="alert">
                  {notice.message}
                </p>
              )}
            </li>
          ))}
        </ul>
      </div>
    </details>
  );
}
