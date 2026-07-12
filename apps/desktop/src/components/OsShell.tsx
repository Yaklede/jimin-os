import {
  BrainCircuit,
  CalendarDays,
  House,
  MessageCircleMore,
  Mic,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import { type ReactNode } from "react";

import { copy } from "../copy";

export type OsDestination =
  "home" | "calendar" | "chat" | "memory" | "settings";

type OsShellProps = {
  destination: OsDestination;
  children: ReactNode;
  rail?: ReactNode;
  onNavigate(destination: OsDestination): void;
  onRefresh(): void;
  refreshing: boolean;
};

export function OsShell({
  destination,
  children,
  rail,
  onNavigate,
  onRefresh,
  refreshing,
}: OsShellProps) {
  const openChat = () => onNavigate("chat");

  return (
    <div className="os-shell" data-destination={destination}>
      <aside className="os-sidebar" aria-label={copy.navigation.label}>
        <button
          className="os-brand focus-visible-control"
          type="button"
          onClick={() => onNavigate("home")}
          aria-label={copy.actions.goHome}
        >
          <span className="os-brand__mark" aria-hidden="true">
            <Sparkles />
          </span>
          <span>{copy.productName.toLocaleLowerCase("en-US")}</span>
        </button>

        <nav className="os-nav" aria-label={copy.navigation.label}>
          <NavigationButton
            active={destination === "home"}
            icon={<House aria-hidden="true" />}
            label={copy.navigation.home}
            onClick={() => onNavigate("home")}
          />
          <NavigationButton
            active={destination === "calendar"}
            icon={<CalendarDays aria-hidden="true" />}
            label={copy.navigation.schedule}
            onClick={() => onNavigate("calendar")}
          />
          <NavigationButton
            active={destination === "memory"}
            icon={<BrainCircuit aria-hidden="true" />}
            label={copy.navigation.memory}
            onClick={() => onNavigate("memory")}
          />
          <NavigationButton
            active={destination === "settings"}
            icon={<Settings2 aria-hidden="true" />}
            label={copy.navigation.settings}
            onClick={() => onNavigate("settings")}
          />
        </nav>

        <button
          className="os-sidebar__assistant focus-visible-control"
          type="button"
          onClick={openChat}
        >
          <Mic aria-hidden="true" />
          {copy.actions.startAssistantConversation}
        </button>
      </aside>

      <section className="os-workspace">
        <header className="os-topbar">
          <button
            className="os-command-launcher focus-visible-control"
            type="button"
            onClick={openChat}
          >
            <Mic aria-hidden="true" />
            <span>{copy.home.commandPlaceholder}</span>
            <kbd>⌘K</kbd>
          </button>
          <div className="os-topbar__controls">
            <time dateTime={new Date().toISOString()}>{todayLabel()}</time>
            <button
              className="os-topbar__refresh focus-visible-control"
              type="button"
              aria-label={copy.actions.refresh}
              onClick={onRefresh}
              disabled={refreshing}
            >
              <RefreshCw
                aria-hidden="true"
                className={refreshing ? "spin" : undefined}
              />
            </button>
          </div>
        </header>
        <main className="os-content">{children}</main>
      </section>

      {rail && <aside className="os-rail">{rail}</aside>}

      <nav className="os-mobile-nav" aria-label={copy.navigation.label}>
        <NavigationButton
          active={destination === "home"}
          icon={<House aria-hidden="true" />}
          label={copy.navigation.mobileHome}
          onClick={() => onNavigate("home")}
        />
        <NavigationButton
          active={destination === "calendar"}
          icon={<CalendarDays aria-hidden="true" />}
          label={copy.navigation.schedule}
          onClick={() => onNavigate("calendar")}
        />
        <button
          className="os-mobile-nav__assistant focus-visible-control"
          type="button"
          aria-label={copy.actions.startAssistantConversation}
          onClick={openChat}
        >
          <Mic aria-hidden="true" />
        </button>
        <NavigationButton
          active={destination === "chat"}
          icon={<MessageCircleMore aria-hidden="true" />}
          label={copy.navigation.chat}
          onClick={openChat}
        />
        <NavigationButton
          active={destination === "settings"}
          icon={<Settings2 aria-hidden="true" />}
          label={copy.navigation.settings}
          onClick={() => onNavigate("settings")}
        />
      </nav>
    </div>
  );
}

function todayLabel(): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
  }).format(new Date());
}

function NavigationButton({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean;
  icon: ReactNode;
  label: string;
  onClick(): void;
}) {
  return (
    <button
      className="os-nav__button focus-visible-control"
      data-active={active}
      type="button"
      onClick={onClick}
      aria-current={active ? "page" : undefined}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
