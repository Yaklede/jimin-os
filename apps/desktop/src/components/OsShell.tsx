import {
  AudioLines,
  BrainCircuit,
  CalendarDays,
  FolderKanban,
  House,
  Inbox,
  Mic,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";

import { copy } from "../copy";
import {
  type VoiceCommandOutcome,
  VoiceCommandSheet,
} from "./VoiceCommandSheet";
import { registerMobileBackHandler } from "../mobileBack";

export type OsDestination =
  | "home"
  | "calendar"
  | "projects"
  | "meetings"
  | "decisions"
  | "chat"
  | "memory"
  | "settings";

type OsShellProps = {
  destination: OsDestination;
  children: ReactNode;
  onNavigate(destination: OsDestination): void;
  onVoiceTranscript(value: string): void;
  onVoiceCommand(value: string): Promise<VoiceCommandOutcome>;
  onRefresh(): void;
  refreshing: boolean;
};

export function OsShell({
  destination,
  children,
  onNavigate,
  onVoiceTranscript,
  onVoiceCommand,
  onRefresh,
  refreshing,
}: OsShellProps) {
  const [voiceSheetOpen, setVoiceSheetOpen] = useState(false);
  const deferredRefreshing = useDeferredBusy(refreshing);
  const previousDestinationRef = useRef(destination);
  const routeDirection = destinationDirection(
    previousDestinationRef.current,
    destination,
  );
  const openChat = () => onNavigate("chat");
  const openVoiceSheet = () => setVoiceSheetOpen(true);

  useEffect(() => {
    previousDestinationRef.current = destination;
  }, [destination]);

  useEffect(() => {
    if (!voiceSheetOpen) return;
    return registerMobileBackHandler(() => {
      setVoiceSheetOpen(false);
      return true;
    }, 100);
  }, [voiceSheetOpen]);

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
            active={destination === "decisions"}
            icon={<Inbox aria-hidden="true" />}
            label={copy.navigation.decisions}
            onClick={() => onNavigate("decisions")}
          />
          <NavigationButton
            active={destination === "meetings"}
            icon={<AudioLines aria-hidden="true" />}
            label={copy.navigation.meetings}
            onClick={() => onNavigate("meetings")}
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
          data-active={deferredRefreshing}
          aria-hidden={!deferredRefreshing}
        >
          <span className="os-page-load__bar" aria-hidden="true" />
          {deferredRefreshing && (
            <span className="sr-only" role="status" aria-live="polite">
              {copy.home.loadingDescription}
            </span>
          )}
        </div>
        <main className="os-content">
          <div
            className="os-content__view"
            data-route-direction={routeDirection}
            key={destination}
          >
            {children}
          </div>
        </main>
      </section>

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
          active={destination === "meetings"}
          icon={<AudioLines aria-hidden="true" />}
          label={copy.navigation.meetings}
          onClick={() => onNavigate("meetings")}
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

function useDeferredBusy(
  busy: boolean,
  delayMs = 180,
  minimumMs = 260,
): boolean {
  const [visible, setVisible] = useState(false);
  const visibleSince = useRef<number | undefined>(undefined);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;

    if (busy && !visible) {
      timer = setTimeout(() => {
        visibleSince.current = Date.now();
        setVisible(true);
      }, delayMs);
    } else if (!busy && visible) {
      const elapsed = Date.now() - (visibleSince.current ?? Date.now());
      timer = setTimeout(
        () => {
          visibleSince.current = undefined;
          setVisible(false);
        },
        Math.max(0, minimumMs - elapsed),
      );
    }

    return () => {
      if (timer) clearTimeout(timer);
    };
  }, [busy, delayMs, minimumMs, visible]);

  return visible;
}

function todayLabel(): string {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "long",
    day: "numeric",
    weekday: "short",
  }).format(new Date());
}

function destinationDirection(
  previous: OsDestination,
  next: OsDestination,
): "backward" | "forward" | "neutral" {
  const order: Partial<Record<OsDestination, number>> = {
    home: 0,
    projects: 1,
    calendar: 2,
    meetings: 3,
  };
  const previousIndex = order[previous];
  const nextIndex = order[next];
  if (previousIndex === undefined || nextIndex === undefined) return "neutral";
  if (previousIndex === nextIndex) return "neutral";
  return nextIndex > previousIndex ? "forward" : "backward";
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
