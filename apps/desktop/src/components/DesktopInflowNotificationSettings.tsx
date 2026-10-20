import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Bell } from "lucide-react";
import {
  desktopInflowNotificationsEnabled,
  desktopInflowNotificationsSupported,
  setDesktopInflowNotificationsEnabled,
  fetchDesktopNotificationDiagnostics,
  testDesktopNotification,
  desktopNotificationConfigurationErrorEvent,
  type DesktopNotificationDiagnostics,
} from "../desktop-inflow-notifications";

export function DesktopInflowNotificationSettings() {
  const [enabled, setEnabled] = useState(desktopInflowNotificationsEnabled);
  const [status, setStatus] = useState("checking");
  const [diagnostics, setDiagnostics] =
    useState<DesktopNotificationDiagnostics>();
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState("");
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    const failed = () => setStatus("invalid");
    window.addEventListener(desktopNotificationConfigurationErrorEvent, failed);
    return () => {
      mounted.current = false;
      window.removeEventListener(
        desktopNotificationConfigurationErrorEvent,
        failed,
      );
    };
  }, []);
  useEffect(() => {
    if (!desktopInflowNotificationsSupported()) return;
    let active = true;
    const registration = listen<string>(
      "inflow-notification-status",
      (event) => {
        if (active) setStatus(event.payload);
      },
    );
    const diagnosticRegistration = listen<DesktopNotificationDiagnostics>(
      "inflow-notification-diagnostics",
      (event) => {
        if (active) setDiagnostics(event.payload);
      },
    );
    void fetchDesktopNotificationDiagnostics()
      .then((value) => {
        if (active) {
          setStatus(value.status);
          setDiagnostics((previous) =>
            (previous?.lastCheckedAt ?? 0) > (value.lastCheckedAt ?? 0)
              ? previous
              : value,
          );
        }
      })
      .catch(() => {
        if (active) setStatus("invalid");
      });
    return () => {
      active = false;
      void registration.then((unlisten) => unlisten());
      void diagnosticRegistration.then((unlisten) => unlisten());
    };
  }, []);
  if (!desktopInflowNotificationsSupported()) return null;
  return (
    <div className="settings-row">
      <span className="settings-row__icon" aria-hidden="true">
        <Bell />
      </span>
      <div className="settings-row__copy">
        <strong>Mac 업무 알림</strong>
        <p>
          {!enabled
            ? "업무 알림을 껐어요."
            : status === "delivery_failed"
              ? "업무는 확인했지만 Mac에 알림을 전달하지 못했어요. 알림 설정을 확인해 주세요. 다음 확인 때 다시 시도해요."
              : status === "checking"
                ? "알림 연결을 확인하고 있어요."
                : status === "offline" ||
                    status === "invalid" ||
                    status === "unauthorized"
                  ? "알림 서버에 연결하지 못했어요. 서버 연결을 확인해 주세요. 연결되면 자동으로 다시 시도해요."
                  : "새 업무 요청과 기존 일감의 새 답글을 알려드려요."}
        </p>
        <p>
          앱이 실행 중이거나 최소화되어 있을 때 받아요. Mac 알림 설정에서 Jimin
          OS를 허용해 주세요.
        </p>
        <details>
          <summary>알림 상태 확인하기</summary>
          <p>
            마지막 서버 확인:{" "}
            {diagnostics?.lastCheckedAt
              ? new Date(diagnostics.lastCheckedAt).toLocaleString("ko-KR")
              : "아직 확인하지 않았어요"}
          </p>
          <p>
            마지막 Mac 전달:{" "}
            {diagnostics?.lastAcceptedAt
              ? new Date(diagnostics.lastAcceptedAt).toLocaleString("ko-KR")
              : "아직 전달 기록이 없어요"}
          </p>
          <p>
            최근 대상 {diagnostics?.eligibleCount ?? 0}개 · 이미 알린 내용{" "}
            {diagnostics?.suppressedCount ?? 0}개
          </p>
          <p>
            내가 보낸 글, 읽거나 처리한 글, 업무로 분류되지 않은 대화는 알리지
            않아요. 확인 위치를 기기에 저장하며, 앱을 다시 열면 같은 계정의 최근
            24시간 안에서 읽지 않은 업무를 이어서 확인해요. 처음 연결하면 지난
            알림을 한꺼번에 보내지 않아요. 실제 표시는 집중 모드·화면 공유·Mac
            알림 설정의 영향을 받아요.
          </p>
          <button
            type="button"
            className="secondary-button focus-visible-control"
            disabled={testing}
            onClick={() => {
              setTesting(true);
              setTestResult("");
              void testDesktopNotification()
                .then(() => {
                  if (mounted.current)
                    setTestResult(
                      "Mac에 테스트 알림을 전달했어요. 보이지 않으면 시스템 알림 설정을 확인해 주세요.",
                    );
                })
                .catch(() => {
                  if (mounted.current)
                    setTestResult(
                      "테스트 알림을 보내지 못했어요. 시스템 알림 설정을 확인하고 다시 시도해 주세요.",
                    );
                })
                .finally(() => {
                  if (mounted.current) setTesting(false);
                });
            }}
          >
            {testing ? "보내는 중" : "테스트 알림 보내기"}
          </button>
          <p role="status">{testResult}</p>
        </details>
      </div>
      <button
        type="button"
        className="text-button"
        aria-pressed={enabled}
        onClick={() => {
          const next = !enabled;
          setEnabled(next);
          setDesktopInflowNotificationsEnabled(next);
        }}
      >
        {enabled ? "알림 끄기" : "알림 켜기"}
      </button>
    </div>
  );
}
