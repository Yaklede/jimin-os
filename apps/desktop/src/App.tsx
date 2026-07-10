import {
  Activity,
  Boxes,
  Database,
  Server,
  SlidersHorizontal,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import {
  fetchServerHealth,
  formatCheckedAt,
  type HealthSnapshot,
} from "./api/health";
import { AppHeader } from "./components/AppHeader";
import {
  ConnectionSummary,
  type ConnectionMode,
} from "./components/ConnectionSummary";
import {
  StatusGroup,
  type RowState,
  type StatusRowData,
} from "./components/StatusGroup";
import { copy } from "./copy";

const apiBaseUrl = import.meta.env.VITE_API_BASE_URL ?? "/server";

type LoadState =
  | { kind: "loading"; snapshot: null }
  | { kind: "refreshing"; snapshot: HealthSnapshot }
  | { kind: "loaded"; snapshot: HealthSnapshot }
  | { kind: "unavailable"; snapshot: null };

export default function App() {
  const [state, setState] = useState<LoadState>({
    kind: "loading",
    snapshot: null,
  });
  const requestSequence = useRef(0);

  const refresh = useCallback(async () => {
    const sequence = requestSequence.current + 1;
    requestSequence.current = sequence;
    setState((current) =>
      current.snapshot
        ? { kind: "refreshing", snapshot: current.snapshot }
        : { kind: "loading", snapshot: null },
    );

    try {
      const snapshot = await fetchServerHealth(apiBaseUrl);
      if (requestSequence.current === sequence) {
        setState({ kind: "loaded", snapshot });
      }
    } catch {
      if (requestSequence.current === sequence) {
        setState({ kind: "unavailable", snapshot: null });
      }
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const mode = resolveConnectionMode(state);
  const readinessRows = useMemo(() => createReadinessRows(state), [state]);
  const isInitialLoading = state.kind === "loading";
  const isRefreshing = state.kind === "loading" || state.kind === "refreshing";
  const snapshot = state.snapshot;

  return (
    <div className="app-shell">
      <AppHeader isRefreshing={isRefreshing} onRefresh={() => void refresh()} />

      <main className="main-content">
        <div className="page-intro">
          <div>
            <p className="eyebrow">{copy.scope}</p>
            <p className="page-intro__title">{copy.pageTitle}</p>
          </div>
          <p>{copy.pageDescription}</p>
        </div>

        <ConnectionSummary mode={mode} />

        <div className="diagnostic-grid">
          <StatusGroup
            title={copy.groups.readinessTitle}
            description={copy.groups.readinessDescription}
            rows={readinessRows}
            loading={isInitialLoading}
          />

          <section
            className="panel panel--details"
            aria-labelledby="server-information-title"
          >
            <div className="panel__header">
              <div>
                <h2 id="server-information-title">{copy.groups.serverTitle}</h2>
                <p>{copy.groups.serverDescription}</p>
              </div>
            </div>
            <dl className="detail-list">
              <DetailRow
                label={copy.details.address}
                value={displayServerAddress(apiBaseUrl)}
              />
              <DetailRow
                label={copy.details.build}
                value={snapshot?.live.buildSha ?? copy.details.waiting}
              />
              <DetailRow
                label={copy.details.structureVersion}
                value={
                  snapshot
                    ? String(snapshot.ready.schemaVersion)
                    : copy.details.waiting
                }
              />
              <DetailRow
                label={copy.details.checkedAt}
                value={
                  snapshot
                    ? formatCheckedAt(snapshot.checkedAt)
                    : copy.details.waiting
                }
              />
            </dl>
          </section>
        </div>

        <p className="page-note">{copy.footer}</p>
      </main>

      <p
        className="sr-only"
        role="status"
        aria-live="polite"
        aria-atomic="true"
      >
        {liveRegionMessage(state, mode)}
      </p>
    </div>
  );
}

interface DetailRowProps {
  label: string;
  value: string;
}

function DetailRow({ label, value }: DetailRowProps) {
  return (
    <div className="detail-row">
      <dt>{label}</dt>
      <dd title={value}>{value}</dd>
    </div>
  );
}

function resolveConnectionMode(state: LoadState): ConnectionMode {
  if (state.kind === "loading") {
    return "checking";
  }
  if (state.kind === "unavailable") {
    return "unavailable";
  }
  return state.snapshot.ready.status === "ready" ? "ready" : "attention";
}

function createReadinessRows(state: LoadState): StatusRowData[] {
  if (state.kind === "unavailable" || state.kind === "loading") {
    return state.kind === "unavailable"
      ? [
          row(
            copy.checks.appResponse,
            copy.checks.appDisconnected,
            "unavailable",
            Activity,
          ),
          row(
            copy.checks.configuration,
            copy.checks.configurationAttention,
            "unavailable",
            SlidersHorizontal,
          ),
          row(
            copy.checks.dataStore,
            copy.checks.dataStoreAttention,
            "unavailable",
            Database,
          ),
          row(
            copy.checks.dataStructure,
            copy.checks.dataStructureAttention,
            "unavailable",
            Boxes,
          ),
        ]
      : [];
  }

  const { checks } = state.snapshot.ready;
  return [
    row(copy.checks.appResponse, copy.checks.appReady, "ready", Activity),
    row(
      copy.checks.configuration,
      checks.configuration === "ok"
        ? copy.checks.configurationReady
        : copy.checks.configurationAttention,
      toRowState(checks.configuration),
      SlidersHorizontal,
    ),
    row(
      copy.checks.dataStore,
      checks.database === "ok"
        ? copy.checks.dataStoreReady
        : copy.checks.dataStoreAttention,
      toRowState(checks.database),
      Database,
    ),
    row(
      copy.checks.dataStructure,
      checks.migrations === "ok"
        ? copy.checks.dataStructureReady
        : copy.checks.dataStructureAttention,
      toRowState(checks.migrations),
      Boxes,
    ),
  ];
}

function row(
  label: string,
  description: string,
  state: RowState,
  Icon: typeof Server,
): StatusRowData {
  return { label, description, state, Icon };
}

function toRowState(value: "ok" | "error"): RowState {
  return value === "ok" ? "ready" : "attention";
}

function displayServerAddress(value: string): string {
  return value.startsWith("/") ? copy.details.localServer : value;
}

function liveRegionMessage(state: LoadState, mode: ConnectionMode): string {
  if (state.kind === "loading" || state.kind === "refreshing") {
    return copy.liveRegion.checking;
  }
  return mode === "unavailable"
    ? copy.liveRegion.disconnected
    : copy.liveRegion[mode];
}
