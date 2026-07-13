import {
  CheckCircle2,
  ChevronRight,
  CircleAlert,
  Link2,
  LoaderCircle,
} from "lucide-react";
import { useEffect, useState } from "react";

import {
  type AgentAuthentication,
  type AgentModelSettings,
} from "../api/agent";
import { copy } from "../copy";

type SettingsWorkspaceProps = {
  authentication: AgentAuthentication | undefined;
  requesting: boolean;
  modelSettings: AgentModelSettings | undefined;
  modelsLoading: boolean;
  modelsSaving: boolean;
  modelsError: string | undefined;
  onStartAuthentication(): Promise<void>;
  onReloadModels(): Promise<void>;
  onSaveModel(modelId: string | null): Promise<boolean>;
};

export function SettingsWorkspace({
  authentication,
  requesting,
  modelSettings,
  modelsLoading,
  modelsSaving,
  modelsError,
  onStartAuthentication,
  onReloadModels,
  onSaveModel,
}: SettingsWorkspaceProps) {
  const savedModelId = modelSettings?.selectedModelId ?? "";
  const [draftModelId, setDraftModelId] = useState(savedModelId);
  const [modelSaved, setModelSaved] = useState(false);
  useEffect(() => {
    setDraftModelId(savedModelId);
  }, [savedModelId]);

  const state = authentication?.state ?? "requested";
  const ready = state === "ready";
  const waiting = state === "requested" || state === "awaiting_authorization";
  const failed = state === "failed";
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
  const waitingForModels = modelsLoading || (!modelSettings && !modelsError);
  const modelChanged = draftModelId !== savedModelId;

  async function saveModel() {
    setModelSaved(false);
    if (await onSaveModel(draftModelId || null)) setModelSaved(true);
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
            <label htmlFor="processing-model">{copy.settings.modelTitle}</label>
            <p id="processing-model-description">
              {copy.settings.modelDescription}
            </p>
          </div>
          <div className="settings-model-field__control">
            <select
              id="processing-model"
              className="focus-visible-control"
              value={draftModelId}
              aria-describedby="processing-model-description processing-model-feedback"
              disabled={waitingForModels || modelsSaving}
              onChange={(event) => {
                setDraftModelId(event.target.value);
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
            <button
              className="primary-button focus-visible-control"
              type="button"
              disabled={waitingForModels || modelsSaving || !modelChanged}
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
                )}
              </span>
            )}
          </div>
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
          <div>
            <strong>{copy.settings.chatgptTitle}</strong>
            <p>{detail}</p>
          </div>
          {ready ? (
            <span className="settings-row__state">
              {copy.settings.chatgptReady}
            </span>
          ) : (
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
      </section>
    </section>
  );
}
