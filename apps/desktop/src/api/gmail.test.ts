import { afterEach, describe, expect, it, vi } from "vitest";

import {
  disconnectGmailAccount,
  fetchGmailAccounts,
  gmailAuthorizationBaseline,
  gmailAuthorizationChanged,
  startGmailAuthorization,
  synchronizeGmailAccount,
  type GmailAccount,
} from "./gmail";

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

const account: GmailAccount = {
  id: "019f68cb-9400-7000-8000-000000000101",
  workspaceId: "019f68cb-9400-7000-8000-000000000201",
  workspaceScope: "company",
  workspaceName: "회사",
  email: "owner@company.example",
  status: "active",
  lastSuccessfulSyncAt: "2026-07-29T05:00:00Z",
  lastErrorCode: null,
  reauthRequired: false,
  canRetryStoredCredential: true,
  version: 2,
};

describe("Gmail accounts client", () => {
  it("loads every workspace-bound account", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify({ available: true, items: [account] }), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      fetchGmailAccounts("https://jimin-os.example/", "access"),
    ).resolves.toEqual({ available: true, items: [account] });
    expect(fetchMock).toHaveBeenCalledWith(
      "https://jimin-os.example/v1/gmail/accounts",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer access" }),
      }),
    );
  });

  it("rejects account metadata that omits stored-credential retry capability", async () => {
    const { canRetryStoredCredential: _, ...incompleteAccount } = account;
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(
          JSON.stringify({ available: true, items: [incompleteAccount] }),
          {
            status: 200,
            headers: { "Content-Type": "application/json" },
          },
        ),
      ),
    );

    await expect(
      fetchGmailAccounts("https://jimin-os.example", "access"),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("starts a workspace-bound authorization on the Google consent host", async () => {
    const authorization = {
      authorizationId: "019f68cb-9400-7000-8000-000000000301",
      authorizationUrl:
        "https://accounts.google.com/o/oauth2/v2/auth?state=safe",
      expiresAt: "2026-07-29T05:10:00Z",
    };
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(authorization), {
        status: 201,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      startGmailAuthorization(
        "https://jimin-os.example",
        "access",
        account.workspaceId,
        { userAgent: "Mozilla/5.0 (Linux; Android 16)" },
      ),
    ).resolves.toEqual(authorization);
    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      clientKind: "android",
      workspaceId: account.workspaceId,
    });
  });

  it("rejects an authorization URL outside Google", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn<typeof fetch>().mockResolvedValue(
        new Response(
          JSON.stringify({
            authorizationId: "019f68cb-9400-7000-8000-000000000301",
            authorizationUrl: "https://example.com/not-google",
            expiresAt: "2026-07-29T05:10:00Z",
          }),
          { status: 201, headers: { "Content-Type": "application/json" } },
        ),
      ),
    );

    await expect(
      startGmailAuthorization(
        "https://jimin-os.example",
        "access",
        account.workspaceId,
      ),
    ).rejects.toMatchObject({ code: "unavailable" });
  });

  it("pins a reconnect authorization to the selected account", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(
        JSON.stringify({
          authorizationId: "019f68cb-9400-7000-8000-000000000301",
          authorizationUrl:
            "https://accounts.google.com/o/oauth2/v2/auth?state=safe",
          expiresAt: "2026-07-29T05:10:00Z",
        }),
        { status: 201, headers: { "Content-Type": "application/json" } },
      ),
    );
    vi.stubGlobal("fetch", fetchMock);

    await startGmailAuthorization(
      "https://jimin-os.example",
      "access",
      account.workspaceId,
      { accountId: account.id, userAgent: "Macintosh" },
    );

    expect(JSON.parse(String(fetchMock.mock.calls[0]?.[1]?.body))).toEqual({
      clientKind: "macos",
      workspaceId: account.workspaceId,
      accountId: account.id,
    });
  });

  it("synchronizes only the selected account", async () => {
    const fetchMock = vi.fn<typeof fetch>().mockResolvedValue(
      new Response(JSON.stringify(account), {
        status: 200,
        headers: { "Content-Type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      synchronizeGmailAccount("https://jimin-os.example", "access", account.id),
    ).resolves.toEqual(account);
    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/gmail/accounts/${account.id}/sync`,
      expect.objectContaining({ method: "POST" }),
    );
  });

  it("disconnects the account version currently shown", async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValue(new Response(null, { status: 204 }));
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      disconnectGmailAccount(
        "https://jimin-os.example/",
        "access",
        account.id,
        account.version,
      ),
    ).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(
      `https://jimin-os.example/v1/gmail/accounts/${account.id}?expectedVersion=2`,
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("rejects an invalid disconnect version before requesting", async () => {
    const fetchMock = vi.fn<typeof fetch>();
    vi.stubGlobal("fetch", fetchMock);

    await expect(
      disconnectGmailAccount(
        "https://jimin-os.example",
        "access",
        account.id,
        0,
      ),
    ).rejects.toMatchObject({ code: "invalid" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("finishes a reconnect when only the selected account changes", () => {
    const sibling = { ...account, id: "sibling", version: 5 };
    const baseline = gmailAuthorizationBaseline(
      account.workspaceId,
      [account, sibling],
      account.id,
    );

    expect(
      gmailAuthorizationChanged(baseline, [
        { ...account, version: account.version + 1 },
        sibling,
      ]),
    ).toBe(true);
    expect(
      gmailAuthorizationChanged(baseline, [
        account,
        { ...sibling, version: sibling.version + 1 },
      ]),
    ).toBe(false);
  });

  it("finishes a reconnect even when the selected account needs attention", () => {
    const baseline = gmailAuthorizationBaseline(
      account.workspaceId,
      [account],
      account.id,
    );

    expect(
      gmailAuthorizationChanged(baseline, [
        {
          ...account,
          status: "reauth_required",
          reauthRequired: true,
          version: account.version + 1,
        },
      ]),
    ).toBe(true);
    expect(
      gmailAuthorizationChanged(baseline, [
        {
          ...account,
          status: "error",
          lastErrorCode: "gmail.provider_unavailable",
          version: account.version + 1,
        },
      ]),
    ).toBe(true);
  });

  it("finishes a new authorization when the resulting account needs attention", () => {
    const baseline = gmailAuthorizationBaseline(account.workspaceId, []);

    expect(
      gmailAuthorizationChanged(baseline, [
        {
          ...account,
          status: "reauth_required",
          reauthRequired: true,
        },
      ]),
    ).toBe(true);
    expect(
      gmailAuthorizationChanged(baseline, [
        {
          ...account,
          id: "019f68cb-9400-7000-8000-000000000102",
          status: "error",
          lastErrorCode: "gmail.provider_unavailable",
        },
      ]),
    ).toBe(true);
  });

  it("keeps waiting while workspace account versions and states are unchanged", () => {
    const baseline = gmailAuthorizationBaseline(account.workspaceId, [account]);

    expect(gmailAuthorizationChanged(baseline, [{ ...account }])).toBe(false);
  });

  it("does not treat a sibling account sync as a new authorization", () => {
    const sibling = { ...account, id: "sibling", version: 5 };
    const baseline = gmailAuthorizationBaseline(account.workspaceId, [
      account,
      sibling,
    ]);

    expect(
      gmailAuthorizationChanged(baseline, [
        account,
        { ...sibling, version: sibling.version + 1 },
      ]),
    ).toBe(false);
  });
});
