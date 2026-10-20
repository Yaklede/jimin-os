import { useEffect, useRef, useState } from "react";
import { ChevronDown } from "lucide-react";
import type { DeliveryRecovery } from "../api/deliveryRecovery";
import { itsmSyncHelp } from "../itsmSync";
import { SafeExternalLink } from "./ExternalTextLink";
import "./inflowRecovery.css";
export function DeliveryRecoveryPanel({
  items,
  onRetry,
}: {
  items: DeliveryRecovery[];
  onRetry(item: DeliveryRecovery): Promise<void>;
}) {
  const [pending, setPending] = useState<string>();
  const [error, setError] = useState("");
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  if (!items.length) return null;
  return (
    <details className="inflow-recovery">
      <summary className="focus-visible-control">
        <span>
          <strong>알림·ITSM 반영 확인하기 · {items.length}개</strong>
        </span>
        <ChevronDown aria-hidden="true" />
      </summary>
      <div className="inflow-recovery__body">
        <p>
          일감은 저장되어 있어요. 알림과 ITSM 반영은 각각 다시 시도할 수 있어요.
          성공한 원문 표시·답글은 다시 보내지 않아요.
        </p>
        {error && (
          <p role="alert" className="inline-alert">
            {error}
          </p>
        )}
        <ul>
          {items.map((item) => (
            <li key={item.id}>
              <strong>{item.title}</strong>
              <p>
                {item.kind === "itsm"
                  ? "ITSM 마감일·완료 상태"
                  : item.kind === "webhook"
                    ? "배정·프로젝트 알림"
                    : "휴대폰 알림"}{" "}
                ·{" "}
                {item.status === "queued" || item.status === "sending"
                  ? "반영을 기다리고 있어요"
                  : item.status === "retry_wait"
                    ? "자동으로 다시 시도하고 있어요"
                    : "전송하지 못했어요"}
              </p>
              {item.kind === "itsm" && item.referenceUrl && (
                <SafeExternalLink href={item.referenceUrl}>
                  연결된 ITSM 티켓 보기
                </SafeExternalLink>
              )}
              {item.kind === "itsm" && item.status === "failed" && (
                <p>{itsmSyncHelp(item.errorCode)}</p>
              )}
              {item.canRetry ? (
                <button
                  type="button"
                  className="secondary-button focus-visible-control"
                  disabled={Boolean(pending)}
                  onClick={() => {
                    setPending(item.id);
                    setError("");
                    void onRetry(item)
                      .catch(() => {
                        if (active.current)
                          setError(
                            "전송을 다시 요청하지 못했어요. 연결 상태를 확인하고 다시 시도해 주세요.",
                          );
                      })
                      .finally(() => {
                        if (active.current) setPending(undefined);
                      });
                  }}
                >
                  {pending === item.id
                    ? "다시 요청하는 중"
                    : item.kind === "itsm"
                      ? "ITSM에 다시 반영하기"
                      : "이 알림만 다시 보내기"}
                </button>
              ) : item.status === "failed" ? (
                <p>
                  기기에서 알림을 다시 허용하거나 연결을 갱신해 주세요. 오래된
                  알림은 다시 보내지 않아요.
                </p>
              ) : null}
            </li>
          ))}
        </ul>
      </div>
    </details>
  );
}
