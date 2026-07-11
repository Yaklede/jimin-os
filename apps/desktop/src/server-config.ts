interface ClientEnvironment {
  readonly DEV?: boolean;
  readonly VITE_API_BASE_URL?: string;
}

export const personalServerBaseUrl = serverBaseUrlFromEnvironment(
  import.meta.env,
);

export function serverBaseUrlFromEnvironment(
  environment: ClientEnvironment,
): string | undefined {
  const configured = environment.VITE_API_BASE_URL?.trim();
  if (configured) return normalizeConfiguredServerUrl(configured);
  return environment.DEV ? "/server" : undefined;
}

function normalizeConfiguredServerUrl(value: string): string | undefined {
  try {
    const url = new URL(value);
    if (
      url.protocol !== "https:" ||
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
