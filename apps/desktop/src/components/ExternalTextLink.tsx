import { type MouseEvent, type ReactNode } from "react";

import { openUrl } from "@tauri-apps/plugin-opener";

type ExternalLinkRuntime = {
  tauri: boolean;
  openTauri(url: string): Promise<void>;
  openWeb(url: string): Window | null;
};

const URL_PATTERN = /https:\/\/[^\s<>"']+/giu;
const TRAILING_PUNCTUATION = /[.,!?;:。，、！？；：」』】]+$/u;
const BRACKET_PAIRS = [
  ["(", ")"],
  ["[", "]"],
  ["{", "}"],
] as const;

function currentRuntime(): ExternalLinkRuntime {
  const tauri =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  return {
    tauri,
    openTauri: openUrl,
    openWeb: (url) => window.open(url, "_blank", "noopener,noreferrer"),
  };
}

export function trustedExternalUrl(value: string): string | undefined {
  try {
    const url = new URL(value.trim());
    return url.protocol === "https:" ? url.toString() : undefined;
  } catch {
    return undefined;
  }
}

export function openTrustedExternalUrl(
  value: string,
  runtime: ExternalLinkRuntime = currentRuntime(),
): Promise<void> {
  const url = trustedExternalUrl(value);
  if (!url) {
    return Promise.reject(new Error("Only HTTPS links can be opened."));
  }
  if (runtime.tauri) {
    return runtime.openTauri(url);
  }
  return runtime.openWeb(url)
    ? Promise.resolve()
    : Promise.reject(new Error("The browser blocked the new window."));
}

export function handleExternalLinkClick(
  event: Pick<MouseEvent<HTMLAnchorElement>, "preventDefault">,
  value: string,
  runtime: ExternalLinkRuntime = currentRuntime(),
) {
  const url = trustedExternalUrl(value);
  if (!url) return;
  event.preventDefault();
  void openTrustedExternalUrl(url, runtime).catch(() => undefined);
}

export function SafeExternalLink({
  href,
  children,
  className,
}: {
  href: string;
  children: ReactNode;
  className?: string;
}) {
  const safeHref = trustedExternalUrl(href);
  if (!safeHref) return <>{children}</>;
  return (
    <a
      className={className}
      href={safeHref}
      target="_blank"
      rel="noreferrer noopener"
      onClick={(event) => handleExternalLinkClick(event, safeHref)}
    >
      {children}
    </a>
  );
}

export function LinkifiedText({ text }: { text: string }) {
  const parts: ReactNode[] = [];
  let cursor = 0;

  for (const match of text.matchAll(URL_PATTERN)) {
    const index = match.index ?? cursor;
    if (index > cursor) parts.push(text.slice(cursor, index));

    const matched = match[0];
    const href = stripTrailingUrlPunctuation(matched);
    const trailing = matched.slice(href.length);
    parts.push(
      <SafeExternalLink
        className="external-text-link"
        href={href}
        key={`${index}-${href}`}
      >
        {href}
      </SafeExternalLink>,
    );
    if (trailing) parts.push(trailing);
    cursor = index + matched.length;
  }

  if (cursor < text.length) parts.push(text.slice(cursor));
  return <>{parts.length > 0 ? parts : text}</>;
}

export function stripTrailingUrlPunctuation(value: string): string {
  let result = value;
  let previous = "";
  while (result !== previous) {
    previous = result;
    result = result.replace(TRAILING_PUNCTUATION, "");
    for (const [opening, closing] of BRACKET_PAIRS) {
      if (!result.endsWith(closing)) continue;
      const openingCount = result.split(opening).length - 1;
      const closingCount = result.split(closing).length - 1;
      if (closingCount > openingCount) result = result.slice(0, -1);
    }
  }
  return result;
}
