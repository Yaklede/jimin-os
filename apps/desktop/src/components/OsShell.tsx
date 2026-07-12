import { House, MessageCircleMore, RefreshCw, Sparkles } from "lucide-react";
import { type ReactNode } from "react";

import { copy } from "../copy";

export type OsDestination = "home" | "assistant";

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
  return (
    <div className="os-shell" data-has-rail={Boolean(rail)}>
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
          <span>Jimin OS</span>
        </button>
        <nav className="os-nav" aria-label={copy.navigation.label}>
          <NavigationButton
            active={destination === "home"}
            icon={<House aria-hidden="true" />}
            label={copy.navigation.home}
            onClick={() => onNavigate("home")}
          />
          <NavigationButton
            active={destination === "assistant"}
            icon={<MessageCircleMore aria-hidden="true" />}
            label={copy.navigation.assistant}
            onClick={() => onNavigate("assistant")}
          />
        </nav>
      </aside>

      <section className="os-workspace">
        <header className="os-topbar">
          <button
            className="os-command-launcher focus-visible-control"
            type="button"
            onClick={() => onNavigate("assistant")}
          >
            <Sparkles aria-hidden="true" />
            <span>{copy.home.commandPlaceholder}</span>
            <kbd>⌘ K</kbd>
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
          label={copy.navigation.home}
          onClick={() => onNavigate("home")}
        />
        <NavigationButton
          active={destination === "assistant"}
          icon={<MessageCircleMore aria-hidden="true" />}
          label={copy.navigation.assistant}
          onClick={() => onNavigate("assistant")}
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
