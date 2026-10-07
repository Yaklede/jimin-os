import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Bell } from "lucide-react";
import {
  desktopInflowNotificationsEnabled,
  desktopInflowNotificationsSupported,
  setDesktopInflowNotificationsEnabled,
} from "../desktop-inflow-notifications";

export function DesktopInflowNotificationSettings() {
  const [enabled, setEnabled] = useState(desktopInflowNotificationsEnabled);
  const [status, setStatus] = useState("checking");
  useEffect(() => {
    if (!desktopInflowNotificationsSupported()) return;
    let active = true;
    const registration = listen<string>(
      "inflow-notification-status",
      (event) => {
        if (active) setStatus(event.payload);
      },
    );
    return () => {
      active = false;
      void registration.then((unlisten) => unlisten());
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
