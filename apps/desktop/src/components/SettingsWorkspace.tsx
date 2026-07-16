import {
  BellRing,
  CalendarDays,
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Clipboard,
  ExternalLink,
  Link2,
  LoaderCircle,
  RefreshCw,
  Unlink,
} from "lucide-react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";

import {
  type AgentAuthentication,
  type AgentModelSettings,
} from "../api/agent";
import { type GoogleCalendarConnection } from "../api/calendar";
import { copy } from "../copy";
import {
  getNotificationPermissionStatus,
  localNotificationsSupported,
  openNotificationSettings,
  type NotificationPermissionStatus,
  type ReminderSyncStatus,
  requestNotificationPermission,
} from "../local-notifications";

type SettingsWorkspaceProps = {
  authentication: AgentAuthentication | undefined;
  requesting: boolean;
  modelSettings: AgentModelSettings | undefined;
  modelsLoading: boolean;
  modelsSaving: boolean;
  modelsError: string | undefined;
  calendarConnection: GoogleCalendarConnection | undefined;
  calendarLoading: boolean;
  calendarAction: "authorizing" | "syncing" | "disconnecting" | undefined;
  calendarAuthorizationPending: boolean;
  calendarError: string | undefined;
  reminderSyncStatus: ReminderSyncStatus;
  reminderSyncError: string | undefined;
  onStartAuthentication(): Promise<void>;
  onReloadModels(): Promise<void>;
  onSaveModel(
    modelId: string | null,
    reasoningEffort: string | null,
  ): Promise<boolean>;
  onStartCalendarConnection(): Promise<void>;
  onReloadCalendarConnection(): Promise<GoogleCalendarConnection | undefined>;
  onSyncCalendar(): Promise<void>;
  onDisconnectCalendar(): Promise<boolean>;
  onRetryReminderSync(): Promise<boolean>;
};

export function SettingsWorkspace({
  authentication,
  requesting,
  modelSettings,
  modelsLoading,
  modelsSaving,
  modelsError,
  calendarConnection,
  calendarLoading,
  calendarAction,
  calendarAuthorizationPending,
  calendarError,
  reminderSyncStatus,
  reminderSyncError,
  onStartAuthentication,
  onReloadModels,
  onSaveModel,
  onStartCalendarConnection,
  onReloadCalendarConnection,
  onSyncCalendar,
  onDisconnectCalendar,
  onRetryReminderSync,
}: SettingsWorkspaceProps) {
  const savedModelId = modelSettings?.selectedModelId ?? "";
  const savedReasoningEffort = modelSettings?.selectedReasoningEffort ?? "";
  const [draftModelId, setDraftModelId] = useState(savedModelId);
  const [draftReasoningEffort, setDraftReasoningEffort] =
    useState(savedReasoningEffort);
  const [modelSaved, setModelSaved] = useState(false);
  const [authenticationCodeCopied, setAuthenticationCodeCopied] =
    useState(false);
  const [authenticationBrowserError, setAuthenticationBrowserError] =
    useState(false);
  const [calendarDisconnectConfirmation, setCalendarDisconnectConfirmation] =
    useState(false);
  const [notificationPermission, setNotificationPermission] =
    useState<NotificationPermissionStatus>();
  const [notificationPermissionLoading, setNotificationPermissionLoading] =
    useState(localNotificationsSupported);
  const [
    notificationPermissionRequesting,
    setNotificationPermissionRequesting,
  ] = useState(false);
  const [notificationPermissionError, setNotificationPermissionError] =
    useState<string>();
  const [notificationSettingsOpening, setNotificationSettingsOpening] =
    useState(false);
  const calendarDisconnectTrigger = useRef<HTMLButtonElement>(null);
  const calendarDisconnectSafeAction = useRef<HTMLButtonElement>(null);
  const calendarConnectionRow = useRef<HTMLDivElement>(null);
  const calendarDisconnectFocusTarget = useRef<"trigger" | "row" | undefined>(
    undefined,
  );
  useEffect(() => {
    setDraftModelId(savedModelId);
    setDraftReasoningEffort(savedReasoningEffort);
  }, [savedModelId, savedReasoningEffort]);

  const state = authentication?.state ?? "requested";
  const ready = state === "ready";
  const waiting = state === "requested" || state === "awaiting_authorization";
  const failed = state === "failed";
  const awaitingAuthorization =
    state === "awaiting_authorization" &&
    Boolean(authentication?.verificationUrl && authentication.userCode);
  const detail = ready
    ? copy.settings.chatgptReady
    : state === "awaiting_authorization"
      ? copy.settings.chatgptAwaiting
      : failed
        ? copy.settings.chatgptFailed
        : waiting
          ? copy.settings.chatgptPreparing
          : copy.settings.chatgptNeedsLogin;
  const defaultModel = modelSettings?.items.find((model) => model.isDefault);
  const draftModel =
    modelSettings?.items.find((model) => model.id === draftModelId) ??
    defaultModel;
  const waitingForModels = modelsLoading || (!modelSettings && !modelsError);
  const settingsChanged =
    draftModelId !== savedModelId ||
    draftReasoningEffort !== savedReasoningEffort;
  const calendarReady = calendarConnection?.status === "active";
  const calendarUnavailable = calendarConnection?.available === false;
  const calendarNeedsAttention =
    calendarConnection?.status === "reauth_required" ||
    calendarConnection?.status === "revoked" ||
    calendarConnection?.status === "error";
  const calendarBusy = calendarLoading || Boolean(calendarAction);
  const calendarDetail = calendarConnectionDetail(
    calendarConnection,
    calendarLoading,
    calendarAuthorizationPending,
  );

  useEffect(() => {
    if (!calendarReady) setCalendarDisconnectConfirmation(false);
  }, [calendarReady]);

  useEffect(() => {
    if (!localNotificationsSupported()) return;
    let active = true;
    let refreshInFlight = false;
    const refreshPermission = async (showLoading: boolean) => {
      if (refreshInFlight) return;
      refreshInFlight = true;
      if (showLoading) setNotificationPermissionLoading(true);
      try {
        const status = await getNotificationPermissionStatus();
        if (!active) return;
        setNotificationPermission((current) =>
          current?.status === status.status &&
          current.canRequest === status.canRequest
            ? current
            : status,
        );
        setNotificationPermissionError(undefined);
      } catch {
        if (!active) return;
        setNotificationPermissionError(copy.settings.notificationsLoadNotice);
      } finally {
        refreshInFlight = false;
        if (active && showLoading) setNotificationPermissionLoading(false);
      }
    };
    const refreshPermissionWhenVisible = () => {
      if (document.visibilityState === "visible") {
        void refreshPermission(false);
      }
    };
    void refreshPermission(true);
    window.addEventListener("focus", refreshPermissionWhenVisible);
    document.addEventListener("visibilitychange", refreshPermissionWhenVisible);
    return () => {
      active = false;
      window.removeEventListener("focus", refreshPermissionWhenVisible);
      document.removeEventListener(
        "visibilitychange",
        refreshPermissionWhenVisible,
      );
    };
  }, []);

  useEffect(() => {
    const target = calendarDisconnectConfirmation
      ? calendarDisconnectSafeAction.current
      : calendarDisconnectFocusTarget.current === "row"
        ? calendarConnectionRow.current
        : calendarDisconnectFocusTarget.current === "trigger"
          ? calendarDisconnectTrigger.current
          : undefined;
    if (!target) return;
    const frame = window.requestAnimationFrame(() => {
      target.focus();
      if (!calendarDisconnectConfirmation) {
        calendarDisconnectFocusTarget.current = undefined;
      }
    });
    return () => window.cancelAnimationFrame(frame);
  }, [calendarDisconnectConfirmation]);

  function closeCalendarDisconnectConfirmation() {
    calendarDisconnectFocusTarget.current = "trigger";
    setCalendarDisconnectConfirmation(false);
  }

  async function askForNotificationPermission() {
    if (notificationPermissionRequesting) return;
    setNotificationPermissionRequesting(true);
    setNotificationPermissionError(undefined);
    try {
      const status = await requestNotificationPermission();
      setNotificationPermission(status);
      if (status.status === "granted") void onRetryReminderSync();
    } catch {
      setNotificationPermissionError(copy.settings.notificationsRequestNotice);
    } finally {
      setNotificationPermissionRequesting(false);
    }
  }

  async function refreshNotificationPermission() {
    if (notificationPermissionLoading) return;
    setNotificationPermissionLoading(true);
    setNotificationPermissionError(undefined);
    try {
      setNotificationPermission(await getNotificationPermissionStatus());
    } catch {
      setNotificationPermissionError(copy.settings.notificationsLoadNotice);
    } finally {
      setNotificationPermissionLoading(false);
    }
  }

  async function openPhoneNotificationSettings() {
    if (notificationSettingsOpening) return;
    setNotificationSettingsOpening(true);
    setNotificationPermissionError(undefined);
    try {
      await openNotificationSettings();
    } catch {
      setNotificationPermissionError(copy.settings.notificationsSettingsNotice);
    } finally {
      setNotificationSettingsOpening(false);
    }
  }

  async function confirmCalendarDisconnect() {
    const disconnected = await onDisconnectCalendar();
    calendarDisconnectFocusTarget.current = disconnected ? "row" : "trigger";
    setCalendarDisconnectConfirmation(false);
  }

  async function saveModel() {
    setModelSaved(false);
    if (await onSaveModel(draftModelId || null, draftReasoningEffort || null)) {
      setModelSaved(true);
    }
  }

  async function copyAuthenticationCode() {
    if (!authentication?.userCode) return;
    try {
      await navigator.clipboard.writeText(authentication.userCode);
      setAuthenticationCodeCopied(true);
    } catch {
      setAuthenticationCodeCopied(false);
    }
  }

  async function openAuthenticationPage() {
    if (!authentication?.verificationUrl) return;
    setAuthenticationBrowserError(false);
    try {
      if (isTauri()) {
        await openUrl(authentication.verificationUrl);
      } else {
        const opened = window.open(
          authentication.verificationUrl,
          "_blank",
          "noopener,noreferrer",
        );
        if (!opened) throw new Error("external navigation unavailable");
      }
    } catch {
      setAuthenticationBrowserError(true);
    }
  }

  return (
    <section className="settings-page">
      <header className="page-heading">
        <p>개인 워크스페이스</p>
        <h1>{copy.settings.title}</h1>
        <span>{copy.settings.description}</span>
      </header>
      <section className="settings-list" aria-label={copy.settings.title}>
        <div className="settings-model-field">
          <div className="settings-model-field__heading">
            <strong>{copy.settings.modelTitle}</strong>
            <p id="processing-model-description">
              {copy.settings.modelDescription}
            </p>
          </div>
          <div className="settings-model-field__control">
            <div className="settings-model-field__option">
              <label htmlFor="processing-model">
                {copy.settings.modelFieldLabel}
              </label>
              <select
                id="processing-model"
                className="focus-visible-control"
                value={draftModelId}
                aria-describedby="processing-model-description processing-model-feedback"
                disabled={waitingForModels || modelsSaving}
                onChange={(event) => {
                  const nextModelId = event.target.value;
                  const nextModel =
                    modelSettings?.items.find(
                      (model) => model.id === nextModelId,
                    ) ?? defaultModel;
                  setDraftModelId(nextModelId);
                  if (
                    draftReasoningEffort &&
                    !nextModel?.supportedReasoningEfforts.some(
                      (effort) => effort.id === draftReasoningEffort,
                    )
                  ) {
                    setDraftReasoningEffort("");
                  }
                  setModelSaved(false);
                }}
              >
                <option value="">
                  {copy.settings.modelAutomatic(defaultModel?.displayName)}
                </option>
                {modelSettings?.items.map((model) => (
                  <option key={model.id} value={model.id}>
                    {model.displayName}
                  </option>
                ))}
              </select>
            </div>
            <div className="settings-model-field__option">
              <label htmlFor="reasoning-effort">
                {copy.settings.effortTitle}
              </label>
              <select
                id="reasoning-effort"
                className="focus-visible-control"
                value={draftReasoningEffort}
                aria-describedby="processing-model-description processing-model-feedback"
                disabled={
                  waitingForModels ||
                  modelsSaving ||
                  !draftModel?.supportedReasoningEfforts.length
                }
                onChange={(event) => {
                  setDraftReasoningEffort(event.target.value);
                  setModelSaved(false);
                }}
              >
                <option value="">
                  {copy.settings.effortAutomatic(
                    draftModel?.defaultReasoningEffort,
                  )}
                </option>
                {draftModel?.supportedReasoningEfforts.map((effort) => (
                  <option key={effort.id} value={effort.id}>
                    {copy.settings.effortLabel(effort.id)}
                  </option>
                ))}
              </select>
            </div>
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={waitingForModels || modelsSaving || !settingsChanged}
              onClick={() => void saveModel()}
            >
              {modelsSaving ? (
                <LoaderCircle className="spin" aria-hidden="true" />
              ) : null}
              {modelsSaving
                ? copy.settings.modelSaving
                : copy.settings.modelSave}
            </button>
          </div>
          <div
            id="processing-model-feedback"
            className="settings-model-field__feedback"
            aria-live="polite"
          >
            {waitingForModels ? (
              <span>{copy.settings.modelLoading}</span>
            ) : modelsError ? (
              <span className="settings-model-field__error" role="alert">
                {modelsError}
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={modelsLoading}
                  onClick={() => void onReloadModels()}
                >
                  {copy.settings.modelReload}
                </button>
              </span>
            ) : modelSettings?.items.length === 0 ? (
              <span>{copy.settings.modelEmpty}</span>
            ) : modelSaved ? (
              <span className="settings-model-field__success">
                <CheckCircle2 aria-hidden="true" />
                {copy.settings.modelSaved}
              </span>
            ) : (
              <span>
                {copy.settings.modelCurrent(
                  modelSettings?.items.find(
                    (model) => model.id === savedModelId,
                  )?.displayName ?? defaultModel?.displayName,
                  copy.settings.effortLabel(
                    modelSettings?.selectedReasoningEffort ??
                      (
                        modelSettings?.items.find(
                          (model) => model.id === savedModelId,
                        ) ?? defaultModel
                      )?.defaultReasoningEffort,
                  ),
                )}
              </span>
            )}
          </div>
        </div>
        <div className="settings-list__section-heading">
          <strong>{copy.settings.connectionsTitle}</strong>
          <p>{copy.settings.connectionsDescription}</p>
        </div>
        <div className="settings-row">
          <span className="settings-row__icon" aria-hidden="true">
            {ready ? (
              <CheckCircle2 />
            ) : failed ? (
              <CircleAlert />
            ) : waiting ? (
              <LoaderCircle className="spin" />
            ) : (
              <Link2 />
            )}
          </span>
          <div className="settings-row__copy">
            <strong>{copy.settings.chatgptTitle}</strong>
            <p>{detail}</p>
            {awaitingAuthorization ? (
              <div className="settings-authentication">
                <div className="settings-authentication__code">
                  <span>{copy.authentication.codeLabel}</span>
                  <output>{authentication?.userCode}</output>
                </div>
                <div className="settings-authentication__actions">
                  <button
                    className="text-button focus-visible-control"
                    type="button"
                    onClick={() => void copyAuthenticationCode()}
                  >
                    <Clipboard aria-hidden="true" />
                    {authenticationCodeCopied
                      ? copy.authentication.copiedCode
                      : copy.actions.copyAuthenticationCode}
                  </button>
                  <button
                    className="text-button focus-visible-control"
                    type="button"
                    onClick={() => void openAuthenticationPage()}
                  >
                    <ExternalLink aria-hidden="true" />
                    {copy.actions.openChatgpt}
                  </button>
                  <button
                    className="text-button focus-visible-control"
                    type="button"
                    disabled={requesting}
                    onClick={() => void onStartAuthentication()}
                  >
                    <RefreshCw aria-hidden="true" />
                    {copy.actions.restartChatgptConnection}
                  </button>
                </div>
                {authenticationBrowserError ? (
                  <p className="settings-row__error" role="alert">
                    {copy.authentication.browserOpenFailed}
                  </p>
                ) : null}
              </div>
            ) : null}
          </div>
          {ready ? (
            <span className="settings-row__state">
              {copy.settings.chatgptReady}
            </span>
          ) : awaitingAuthorization ? null : (
            <button
              className="text-button focus-visible-control"
              type="button"
              onClick={() => void onStartAuthentication()}
              disabled={requesting || waiting}
            >
              {failed
                ? copy.actions.retryChatgptConnection
                : copy.actions.connectChatgpt}
              <ChevronRight aria-hidden="true" />
            </button>
          )}
        </div>
        <div
          ref={calendarConnectionRow}
          className="settings-row focus-visible-control"
          tabIndex={-1}
          aria-busy={calendarBusy}
          data-state={
            calendarUnavailable ? "unavailable" : calendarConnection?.status
          }
        >
          <span className="settings-row__icon" aria-hidden="true">
            {calendarBusy ? (
              <LoaderCircle className="spin" />
            ) : calendarReady ? (
              <CheckCircle2 />
            ) : calendarUnavailable || calendarNeedsAttention ? (
              <CircleAlert />
            ) : (
              <CalendarDays />
            )}
          </span>
          <div className="settings-row__copy">
            <strong>{copy.settings.calendarTitle}</strong>
            <p>{calendarDetail}</p>
            {calendarError && (
              <p className="settings-row__error" role="alert">
                {calendarError}
              </p>
            )}
          </div>
          <div className="settings-row__actions">
            {calendarUnavailable ? (
              <span className="settings-row__state settings-row__state--warning">
                {copy.settings.calendarConfigurationRequired}
              </span>
            ) : !calendarConnection ? (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarLoading}
                onClick={() => void onReloadCalendarConnection()}
              >
                {calendarLoading ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : (
                  <RefreshCw aria-hidden="true" />
                )}
                {calendarLoading
                  ? copy.settings.calendarChecking
                  : copy.settings.calendarRetry}
              </button>
            ) : calendarReady ? (
              <>
                {!calendarDisconnectConfirmation ? (
                  <button
                    className="text-button focus-visible-control"
                    type="button"
                    disabled={calendarBusy}
                    onClick={() => void onSyncCalendar()}
                  >
                    {calendarAction === "syncing" ? (
                      <LoaderCircle className="spin" aria-hidden="true" />
                    ) : (
                      <RefreshCw aria-hidden="true" />
                    )}
                    {calendarAction === "syncing"
                      ? copy.settings.calendarSyncing
                      : copy.settings.calendarSync}
                  </button>
                ) : null}
                {calendarDisconnectConfirmation ? (
                  <div
                    className="settings-row__disconnect-confirmation"
                    role="group"
                    aria-label={copy.settings.calendarDisconnectTitle}
                  >
                    <p>{copy.settings.calendarDisconnectDescription}</p>
                    <div>
                      <button
                        ref={calendarDisconnectSafeAction}
                        className="text-button focus-visible-control"
                        type="button"
                        disabled={calendarBusy}
                        onClick={closeCalendarDisconnectConfirmation}
                      >
                        {copy.settings.calendarKeepConnected}
                      </button>
                      <button
                        className="text-button text-button--danger focus-visible-control"
                        type="button"
                        disabled={calendarBusy}
                        onClick={() => void confirmCalendarDisconnect()}
                      >
                        {calendarAction === "disconnecting" ? (
                          <LoaderCircle className="spin" aria-hidden="true" />
                        ) : (
                          <Unlink aria-hidden="true" />
                        )}
                        {calendarAction === "disconnecting"
                          ? copy.settings.calendarDisconnectingAction
                          : copy.settings.calendarConfirmDisconnect}
                      </button>
                    </div>
                  </div>
                ) : (
                  <button
                    ref={calendarDisconnectTrigger}
                    className="text-button text-button--danger focus-visible-control"
                    type="button"
                    disabled={calendarBusy}
                    onClick={() => setCalendarDisconnectConfirmation(true)}
                  >
                    <Unlink aria-hidden="true" />
                    {copy.settings.calendarDisconnect}
                  </button>
                )}
              </>
            ) : calendarAuthorizationPending ? (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarBusy}
                onClick={() => void onReloadCalendarConnection()}
              >
                {calendarLoading ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : null}
                {calendarLoading
                  ? copy.settings.calendarChecking
                  : copy.settings.calendarCheckConnection}
              </button>
            ) : (
              <button
                className="text-button focus-visible-control"
                type="button"
                disabled={calendarBusy}
                onClick={() => void onStartCalendarConnection()}
              >
                {calendarAction === "authorizing" ? (
                  <LoaderCircle className="spin" aria-hidden="true" />
                ) : null}
                {calendarAction === "authorizing"
                  ? copy.settings.calendarOpening
                  : calendarNeedsAttention
                    ? copy.settings.calendarReconnect
                    : copy.settings.calendarConnect}
                {!calendarBusy && <ChevronRight aria-hidden="true" />}
              </button>
            )}
          </div>
        </div>
        {localNotificationsSupported() ? (
          <div
            className="settings-row"
            aria-busy={
              notificationPermissionLoading ||
              notificationPermissionRequesting ||
              notificationSettingsOpening ||
              reminderSyncStatus === "syncing"
            }
            data-state={
              notificationPermissionError || reminderSyncError
                ? "error"
                : notificationPermission?.status
            }
          >
            <span className="settings-row__icon" aria-hidden="true">
              {notificationPermissionLoading ||
              notificationPermissionRequesting ? (
                <LoaderCircle className="spin" />
              ) : notificationPermissionError ||
                reminderSyncError ||
                notificationPermission?.status === "denied" ? (
                <CircleAlert />
              ) : notificationPermission?.status === "granted" ? (
                <CheckCircle2 />
              ) : (
                <BellRing />
              )}
            </span>
            <div className="settings-row__copy">
              <strong>{copy.settings.notificationsTitle}</strong>
              <p>
                {notificationPermissionLoading
                  ? copy.settings.notificationsChecking
                  : notificationPermission?.status === "granted"
                    ? reminderSyncStatus === "syncing" ||
                      reminderSyncStatus === "idle"
                      ? copy.settings.notificationsSyncing
                      : reminderSyncStatus === "error"
                        ? copy.settings.notificationsSyncProblem
                        : copy.settings.notificationsReady
                    : notificationPermission?.canRequest
                      ? copy.settings.notificationsNeedsPermission
                      : copy.settings.notificationsNeedsSettings}
              </p>
              {notificationPermissionError ? (
                <p className="settings-row__error" role="alert">
                  {notificationPermissionError}
                </p>
              ) : null}
              {!notificationPermissionError && reminderSyncError ? (
                <p className="settings-row__error" role="alert">
                  {reminderSyncError}
                </p>
              ) : null}
            </div>
            <div className="settings-row__actions">
              {notificationPermissionError ? (
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={notificationPermissionLoading}
                  onClick={() => void refreshNotificationPermission()}
                >
                  {notificationPermissionLoading ? (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  ) : (
                    <RefreshCw aria-hidden="true" />
                  )}
                  {copy.settings.notificationsRetry}
                </button>
              ) : notificationPermission?.status === "granted" ? (
                reminderSyncStatus === "error" ? (
                  <button
                    className="text-button focus-visible-control"
                    type="button"
                    onClick={() => void onRetryReminderSync()}
                  >
                    <RefreshCw aria-hidden="true" />
                    {copy.settings.notificationsSyncRetry}
                  </button>
                ) : (
                  <span className="settings-row__state" role="status">
                    {reminderSyncStatus === "syncing" ||
                    reminderSyncStatus === "idle" ? (
                      <LoaderCircle className="spin" aria-hidden="true" />
                    ) : (
                      <CheckCircle2 aria-hidden="true" />
                    )}
                    {reminderSyncStatus === "syncing" ||
                    reminderSyncStatus === "idle"
                      ? copy.settings.notificationsSyncingAction
                      : copy.settings.notificationsEnabled}
                  </span>
                )
              ) : notificationPermission?.canRequest ? (
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={notificationPermissionRequesting}
                  onClick={() => void askForNotificationPermission()}
                >
                  {notificationPermissionRequesting ? (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  ) : (
                    <BellRing aria-hidden="true" />
                  )}
                  {notificationPermissionRequesting
                    ? copy.settings.notificationsRequesting
                    : copy.settings.notificationsAllow}
                </button>
              ) : notificationPermission?.status === "denied" ? (
                <button
                  className="text-button focus-visible-control"
                  type="button"
                  disabled={notificationSettingsOpening}
                  onClick={() => void openPhoneNotificationSettings()}
                >
                  {notificationSettingsOpening ? (
                    <LoaderCircle className="spin" aria-hidden="true" />
                  ) : (
                    <BellRing aria-hidden="true" />
                  )}
                  {notificationSettingsOpening
                    ? copy.settings.notificationsOpeningSettings
                    : copy.settings.notificationsOpenSettings}
                </button>
              ) : null}
            </div>
          </div>
        ) : null}
      </section>
    </section>
  );
}

function calendarConnectionDetail(
  connection: GoogleCalendarConnection | undefined,
  loading: boolean,
  authorizationPending: boolean,
): string {
  if (!connection && loading) return copy.settings.calendarLoading;
  if (!connection) return copy.settings.calendarLoadFailed;
  if (!connection.available) return copy.settings.calendarConfigurationMissing;
  if (authorizationPending && connection.status !== "active") {
    return copy.settings.calendarAwaitingAuthorization;
  }
  if (connection.status === "active") {
    return copy.settings.calendarConnected(
      connection.email ?? undefined,
      connection.lastSuccessfulSyncAt ?? undefined,
    );
  }
  if (connection.status === "reauth_required") {
    return copy.settings.calendarReauthRequired;
  }
  if (connection.status === "connecting") {
    return copy.settings.calendarAwaitingAuthorization;
  }
  if (connection.status === "revoking") {
    return copy.settings.calendarDisconnecting;
  }
  if (connection.status === "revoked" || connection.status === "error") {
    return copy.settings.calendarNeedsReconnect;
  }
  return copy.settings.calendarNotConnected;
}
