import {
  AudioLines,
  BrainCircuit,
  CalendarDays,
  FolderKanban,
  House,
  Inbox,
  Mic,
  MoreHorizontal,
  RefreshCw,
  Settings2,
  Sparkles,
} from "lucide-react";
import {
  lazy,
  type ReactNode,
  Suspense,
  useEffect,
  useRef,
  useState,
} from "react";

import { copy } from "../copy";
import { useMobileViewport } from "../useMobileLayout";
import { AppearanceControl } from "./appearance-control";
import { RepresentativeImage } from "./RepresentativeImage";
import { type VoiceCommandOutcome } from "./VoiceCommandSheet";
import { registerMobileBackHandler } from "../mobileBack";
import {
  mobileCapabilitySnapshot,
  type MobilePlatform,
} from "../mobile-capabilities";

const VoiceCommandSheet = lazy(() =>
  import("./VoiceCommandSheet").then((module) => ({
    default: module.VoiceCommandSheet,
  })),
);

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
  platform?: MobilePlatform;
};

export function OsShell({
  destination,
  children,
  onNavigate,
  onVoiceTranscript,
  onVoiceCommand,
  onRefresh,
  refreshing,
  platform: platformOverride,
}: OsShellProps) {
  useMobileViewport();
  const navigateFromSidebar = (target: OsDestination) => onNavigate(target);
  const [voiceSheetOpen, setVoiceSheetOpen] = useState(false);
  const [runtimePlatform] = useState(
    () => platformOverride ?? mobileCapabilitySnapshot().platform,
  );
  const deferredRefreshing = useDeferredBusy(refreshing);
  const previousDestinationRef = useRef(destination);
  const mobileMoreRef = useRef<HTMLDetailsElement>(null);
  const routeDirection = destinationDirection(
    previousDestinationRef.current,
    destination,
  );
  const openChat = () => onNavigate("chat");
  const openVoiceSheet = () => setVoiceSheetOpen(true);

  useEffect(() => {
    previousDestinationRef.current = destination;
    if (mobileMoreRef.current) mobileMoreRef.current.open = false;
  }, [destination]);

  useEffect(() => {
    if (!voiceSheetOpen) return;
    return registerMobileBackHandler(() => {
      setVoiceSheetOpen(false);
      return true;
    }, 100);
  }, [voiceSheetOpen]);

  useEffect(
    () =>
      registerMobileBackHandler(() => {
        if (!mobileMoreRef.current?.open) return false;
        mobileMoreRef.current.open = false;
        mobileMoreRef.current.querySelector("summary")?.focus();
        return true;
      }, 80),
    [],
  );

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
    <div
      className="os-shell"
      data-destination={destination}
      data-platform={runtimePlatform}
    >
      <aside className="os-sidebar" aria-label={copy.navigation.label}>
        <button
          className="os-brand focus-visible-control"
          type="button"
          onClick={() => navigateFromSidebar("home")}
          aria-label={copy.actions.goHome}
        >
          <span
            className="os-brand__mark os-brand__mark--hamster"
            aria-hidden="true"
          >
            <RepresentativeImage />
          </span>
          <span>{copy.productName.toLocaleLowerCase("en-US")}</span>
        </button>

        <nav
          id="primary-navigation"
          className="os-nav"
          aria-label={copy.navigation.label}
        >
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
            onClick={() => navigateFromSidebar("projects")}
          />
          <NavigationButton
            active={destination === "decisions"}
            icon={<Inbox aria-hidden="true" />}
            label={copy.navigation.decisions}
            onClick={() => navigateFromSidebar("decisions")}
          />
          <NavigationButton
            active={destination === "meetings"}
            icon={<AudioLines aria-hidden="true" />}
            label={copy.navigation.meetings}
            onClick={() => navigateFromSidebar("meetings")}
          />
          <NavigationButton
            active={destination === "memory"}
            icon={<BrainCircuit aria-hidden="true" />}
            label={copy.navigation.memory}
            onClick={() => navigateFromSidebar("memory")}
          />
          <NavigationButton
            active={destination === "settings"}
            icon={<Settings2 aria-hidden="true" />}
            label={copy.navigation.settings}
            onClick={() => navigateFromSidebar("settings")}
          />
        </nav>

        <button
          className="os-sidebar__assistant focus-visible-control"
          type="button"
          onClick={() => navigateFromSidebar("chat")}
          aria-label={copy.actions.startAssistantConversation}
        >
          <Mic aria-hidden="true" />
          <span>{copy.actions.startAssistantConversation}</span>
        </button>
      </aside>

      <section className="os-workspace">
        <header className="os-topbar">
          <div className="os-topbar__inner">
            <button
              className="os-command-launcher focus-visible-control"
              type="button"
              onClick={
                runtimePlatform === "android" || runtimePlatform === "ios"
                  ? openVoiceSheet
                  : openChat
              }
            >
              <Mic aria-hidden="true" />
              <span>{copy.home.commandPlaceholder}</span>
              <kbd>⌘K</kbd>
            </button>
            <time
              className="os-topbar__date"
              dateTime={new Date().toISOString()}
            >
              {todayLabel()}
            </time>
            <div className="os-topbar__controls">
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
              <AppearanceControl />
            </div>
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
        <details
          className="os-mobile-more"
          ref={mobileMoreRef}
          onKeyDown={(event) => {
            if (event.key !== "Escape") return;
            event.currentTarget.open = false;
            event.currentTarget.querySelector("summary")?.focus();
          }}
        >
          <summary
            className="os-nav__button focus-visible-control"
            data-active={["decisions", "memory", "settings"].includes(
              destination,
            )}
          >
            <MoreHorizontal aria-hidden="true" />
            <span>{copy.navigation.more}</span>
          </summary>
          <div className="os-mobile-more__items">
            {(
              [
                [
                  "decisions",
                  copy.navigation.decisions,
                  <Inbox aria-hidden="true" />,
                ],
                [
                  "memory",
                  copy.navigation.memory,
                  <BrainCircuit aria-hidden="true" />,
                ],
                [
                  "settings",
                  copy.navigation.settings,
                  <Settings2 aria-hidden="true" />,
                ],
              ] as const
            ).map(([target, label, icon]) => (
              <NavigationButton
                key={target}
                active={destination === target}
                icon={icon}
                label={label}
                onClick={() => {
                  if (mobileMoreRef.current) mobileMoreRef.current.open = false;
                  onNavigate(target);
                }}
              />
            ))}
          </div>
        </details>
      </nav>

      {voiceSheetOpen && (
        <Suspense fallback={null}>
          <VoiceCommandSheet
            open
            onClose={() => setVoiceSheetOpen(false)}
            onOpenTextInput={openTextInput}
            onOpenDestination={openVoiceDestination}
            onProcessTranscript={onVoiceCommand}
          />
        </Suspense>
      )}
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
