interface ClientEnvironment {
  readonly DEV?: boolean;
  readonly VITE_API_BASE_URL?: string;
  readonly VITE_LOCAL_PHONE_TEST?: string;
}

export const personalServerBaseUrl = serverBaseUrlFromEnvironment(
  import.meta.env,
);

export function serverBaseUrlFromEnvironment(
  environment: ClientEnvironment,
): string | undefined {
  const configured = environment.VITE_API_BASE_URL?.trim();
  if (configured) {
    return normalizeConfiguredServerUrl(
      configured,
      environment.VITE_LOCAL_PHONE_TEST === "1",
    );
  }
  return environment.DEV ? "/server" : undefined;
}

function normalizeConfiguredServerUrl(
  value: string,
  allowLoopbackHttp: boolean,
): string | undefined {
  try {
    const url = new URL(value);
    const isLocalPhoneTestOrigin =
      allowLoopbackHttp &&
      url.protocol === "http:" &&
      url.hostname === "127.0.0.1" &&
      url.port === "8080";
    if (
      (url.protocol !== "https:" && !isLocalPhoneTestOrigin) ||
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      return undefined;
    }
    return url.origin;
  } catch {
    return undefined;
  }
}
