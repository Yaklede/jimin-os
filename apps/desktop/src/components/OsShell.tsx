import {
  BrainCircuit,
  CalendarDays,
  FolderKanban,
  House,
  Mic,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import { type ReactNode, useState } from "react";

import { copy } from "../copy";
import {
  type VoiceCommandOutcome,
  VoiceCommandSheet,
} from "./VoiceCommandSheet";

export type OsDestination =
  "home" | "calendar" | "projects" | "chat" | "memory" | "settings";

type OsShellProps = {
  destination: OsDestination;
  children: ReactNode;
  rail?: ReactNode;
  onNavigate(destination: OsDestination): void;
  onVoiceTranscript(value: string): void;
  onVoiceCommand(value: string): Promise<VoiceCommandOutcome>;
  onRefresh(): void;
  refreshing: boolean;
};

export function OsShell({
  destination,
  children,
  rail,
  onNavigate,
  onVoiceTranscript,
  onVoiceCommand,
  onRefresh,
  refreshing,
}: OsShellProps) {
  const [voiceSheetOpen, setVoiceSheetOpen] = useState(false);
  const openChat = () => onNavigate("chat");
  const openVoiceSheet = () => setVoiceSheetOpen(true);

  function openTextInput(value?: string) {
    setVoiceSheetOpen(false);
    if (value) {
      onVoiceTranscript(value);
      return;
    }
    openChat();
  }

  function openVoiceDestination(destination: "home" | "calendar") {
    setVoiceSheetOpen(false);
    onNavigate(destination);
  }

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
            active={destination === "projects"}
            icon={<FolderKanban aria-hidden="true" />}
            label={copy.navigation.projects}
            onClick={() => onNavigate("projects")}
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
          aria-label={copy.actions.startAssistantConversation}
        >
          <Mic aria-hidden="true" />
          <span>{copy.actions.startAssistantConversation}</span>
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
                className={refreshing ? "spin" : ""}
              />
            </button>
          </div>
        </header>
        <div
          className="os-page-load"
          data-active={refreshing}
          aria-hidden={!refreshing}
        >
          <span className="os-page-load__bar" aria-hidden="true" />
          {refreshing && (
            <span className="sr-only" role="status" aria-live="polite">
              {copy.home.loadingDescription}
            </span>
          )}
        </div>
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
          active={destination === "projects"}
          icon={<FolderKanban aria-hidden="true" />}
          label={copy.navigation.projects}
          onClick={() => onNavigate("projects")}
        />
        <button
          className="os-mobile-nav__assistant focus-visible-control"
          type="button"
          aria-label={copy.actions.startAssistantConversation}
          onClick={openVoiceSheet}
        >
          <Mic aria-hidden="true" />
        </button>
        <NavigationButton
          active={destination === "calendar"}
          icon={<CalendarDays aria-hidden="true" />}
          label={copy.navigation.schedule}
          onClick={() => onNavigate("calendar")}
        />
        <NavigationButton
          active={destination === "settings"}
          icon={<Settings2 aria-hidden="true" />}
          label={copy.navigation.settings}
          onClick={() => onNavigate("settings")}
        />
      </nav>

      <VoiceCommandSheet
        open={voiceSheetOpen}
        onClose={() => setVoiceSheetOpen(false)}
        onOpenTextInput={openTextInput}
        onOpenDestination={openVoiceDestination}
        onProcessTranscript={onVoiceCommand}
      />
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
      aria-label={label}
      {...(active ? { "aria-current": "page" as const } : {})}
    >
      {icon}
      <span>{label}</span>
    </button>
  );
}
