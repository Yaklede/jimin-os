import { createElement, type ComponentProps } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import type { GmailAccount } from "../api/gmail";
import type { Workspace } from "../api/projects";
import { copy } from "../copy";
import {
  gmailAccountActionActive,
  gmailAccountDetail,
  groupGmailAccounts,
  SettingsWorkspace,
} from "./SettingsWorkspace";

const workspaces: Workspace[] = [
  {
    id: "company-workspace",
    scope: "company",
    name: "회사",
    version: 1,
  },
  {
    id: "personal-workspace",
    scope: "personal",
    name: "개인",
    version: 1,
  },
];

function account(
  overrides: Partial<GmailAccount> & Pick<GmailAccount, "id" | "workspaceId">,
): GmailAccount {
  return {
    workspaceScope:
      overrides.workspaceId === "personal-workspace" ? "personal" : "company",
    workspaceName:
      overrides.workspaceId === "personal-workspace" ? "개인" : "회사",
    email: `${overrides.id}@example.com`,
    status: "active",
    lastSuccessfulSyncAt: "2026-07-29T05:00:00Z",
    lastErrorCode: null,
    reauthRequired: false,
    version: 1,
    ...overrides,
  };
}

describe("Gmail settings presentation", () => {
  it("keeps personal and company accounts in separate workspace groups", () => {
    const groups = groupGmailAccounts(workspaces, [
      account({ id: "company", workspaceId: "company-workspace" }),
      account({ id: "personal", workspaceId: "personal-workspace" }),
    ]);

    expect(groups.map((group) => group.workspace.scope)).toEqual([
      "personal",
      "company",
    ]);
    expect(groups[0]?.accounts.map((item) => item.id)).toEqual(["personal"]);
    expect(groups[1]?.accounts.map((item) => item.id)).toEqual(["company"]);
  });

  it("does not hide a connected account while workspace data is refreshing", () => {
    const groups = groupGmailAccounts(
      [],
      [
        account({
          id: "orphan",
          workspaceId: "company-workspace",
          workspaceName: "회사",
        }),
      ],
    );

    expect(groups).toHaveLength(1);
    expect(groups[0]?.workspace).toEqual({
      id: "company-workspace",
      scope: "company",
      name: "회사",
    });
    expect(groups[0]?.accounts[0]?.id).toBe("orphan");
  });

  it("gives an actionable detail when Google permission needs attention", () => {
    expect(
      gmailAccountDetail(
        account({
          id: "reauth",
          workspaceId: "company-workspace",
          status: "reauth_required",
          reauthRequired: true,
        }),
      ),
    ).toContain("다시 연결");
  });

  it("gives an actionable detail when synchronization failed", () => {
    expect(
      gmailAccountDetail(
        account({
          id: "failed",
          workspaceId: "company-workspace",
          lastErrorCode: "provider_unavailable",
        }),
      ),
    ).toContain("다시 눌러");
  });

  it("keeps an account action independent from other account cards", () => {
    const actions = [{ kind: "syncing" as const, accountId: "company" }];

    expect(gmailAccountActionActive(actions, "company", "syncing")).toBe(true);
    expect(gmailAccountActionActive(actions, "personal")).toBe(false);
  });

  it("keeps Calendar visible while Gmail is loading", () => {
    const markup = renderSettings({ gmailLoading: true });

    expect(markup).toContain("Google Calendar");
    expect(markup).toContain("Gmail 계정을 확인하고 있어요.");
  });

  it("explains that disconnecting Calendar keeps Gmail accounts", () => {
    expect(copy.settings.calendarDisconnectDescription).toContain(
      "Gmail 계정은 그대로",
    );
    expect(copy.settings.calendarDisconnectDescription).not.toContain(
      "메일 요약은 지워지고",
    );
  });

  it("shows an empty workspace action instead of mixing account scopes", () => {
    const markup = renderSettings();

    expect(markup).toContain("개인");
    expect(markup).toContain("아직 연결한 Gmail 계정이 없어요.");
    expect(markup).toContain("계정 추가하기");
  });

  it("shows the next action for account load and reauthorization failures", () => {
    const loadFailure = renderSettings({
      gmailError:
        "Gmail 계정을 불러오지 못했어요. 서버 연결을 확인한 뒤 다시 시도해 주세요.",
    });
    const reconnect = renderSettings({
      gmailAccounts: [
        account({
          id: "reauth",
          workspaceId: "personal-workspace",
          status: "reauth_required",
          reauthRequired: true,
        }),
      ],
    });

    expect(loadFailure).toContain("계정 다시 확인하기");
    expect(reconnect).toContain("다시 연결하기");
  });

  it("keeps account actions visible but disabled while Gmail is unavailable", () => {
    const markup = renderSettings({
      gmailAvailable: false,
      gmailAccounts: [
        account({
          id: "connected",
          workspaceId: "personal-workspace",
        }),
        account({
          id: "reauth",
          workspaceId: "personal-workspace",
          status: "reauth_required",
          reauthRequired: true,
        }),
      ],
    });

    expect(markup).toContain(copy.settings.gmailConfigurationRequired);
    expect(buttonOpeningTag(markup, copy.settings.gmailAddAccount)).toContain(
      "disabled",
    );
    expect(buttonOpeningTag(markup, copy.settings.gmailSync)).toContain(
      "disabled",
    );
    expect(buttonOpeningTag(markup, copy.settings.gmailReconnect)).toContain(
      "disabled",
    );
  });

  it("lets the user cancel a pending authorization immediately", () => {
    const markup = renderSettings({
      gmailAuthorizationPendingWorkspaceId: "personal-workspace",
    });

    expect(markup).toContain(copy.settings.gmailAwaitingTitle);
    expect(markup).toContain(copy.settings.gmailCancelConnection);
    expect(
      buttonOpeningTag(markup, copy.settings.gmailCancelConnection),
    ).not.toContain("disabled");
  });
});

function buttonOpeningTag(markup: string, label: string): string {
  const labelIndex = markup.indexOf(label);
  expect(labelIndex).toBeGreaterThanOrEqual(0);
  const buttonIndex = markup.lastIndexOf("<button", labelIndex);
  expect(buttonIndex).toBeGreaterThanOrEqual(0);
  return markup.slice(buttonIndex, markup.indexOf(">", buttonIndex) + 1);
}

function renderSettings(
  overrides: Partial<ComponentProps<typeof SettingsWorkspace>> = {},
): string {
  const props: ComponentProps<typeof SettingsWorkspace> = {
    authentication: {
      state: "ready",
      verificationUrl: null,
      userCode: null,
    },
    requesting: false,
    modelSettings: undefined,
    modelsLoading: false,
    modelsSaving: false,
    modelsError: undefined,
    calendarConnection: {
      available: true,
      status: "active",
      email: "calendar@example.com",
      grantedScopes: [],
      lastSuccessfulSyncAt: null,
      lastErrorCode: null,
      reauthRequired: false,
      version: 1,
    },
    calendarLoading: false,
    calendarAction: undefined,
    calendarAuthorizationPending: false,
    calendarError: undefined,
    workspaces: [workspaces[1]!],
    gmailAvailable: true,
    gmailAccounts: [],
    gmailLoading: false,
    gmailActions: [],
    gmailAuthorizationPendingWorkspaceId: undefined,
    gmailError: undefined,
    reminderSyncStatus: "ready",
    reminderSyncError: undefined,
    remoteReminderStatus: "connected",
    deviceSignalStates: [],
    nativeCallLogPermission: undefined,
    deviceSignalsLoading: false,
    deviceSignalsError: undefined,
    onStartAuthentication: async () => undefined,
    onReloadModels: async () => undefined,
    onSaveModel: async () => true,
    onStartCalendarConnection: async () => undefined,
    onReloadCalendarConnection: async () => undefined,
    onSyncCalendar: async () => undefined,
    onDisconnectCalendar: async () => true,
    onReloadGmailAccounts: async () => [],
    onStartGmailConnection: async () => undefined,
    onCancelGmailAuthorization: () => undefined,
    onSyncGmailAccount: async () => undefined,
    onDisconnectGmailAccount: async () => true,
    onRetryReminderSync: async () => true,
    onEnableDeviceSignals: async () => true,
    onRefreshDeviceSignals: async () => true,
    ...overrides,
  };
  return renderToStaticMarkup(createElement(SettingsWorkspace, props));
}
