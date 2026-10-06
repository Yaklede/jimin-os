export type AppearanceMode = "light" | "dark";
export type AppearanceColor = "yellow" | "mint" | "blue" | "purple" | "pink";
export type AppearancePreferences = {
  mode: AppearanceMode;
  color: AppearanceColor;
};
type PreferenceStorage = Pick<Storage, "getItem" | "setItem">;

export const appearanceStorageKey = "jimin-os.appearance.v1";
export const appearanceColors: AppearanceColor[] = [
  "yellow",
  "mint",
  "blue",
  "purple",
  "pink",
];
export const appearancePalettes: Record<
  AppearanceColor,
  Record<
    AppearanceMode,
    {
      accent: string;
      hover: string;
      onAccent: string;
      label: string;
    }
  >
> = {
  yellow: {
    light: {
      accent: "#ffd43b",
      hover: "#f5be18",
      onAccent: "#222222",
      label: "#805400",
    },
    dark: {
      accent: "#ffdf93",
      hover: "#ffe8b2",
      onAccent: "#222222",
      label: "#ffdf93",
    },
  },
  mint: {
    light: {
      accent: "#7affdc",
      hover: "#55ecc8",
      onAccent: "#24292e",
      label: "#005e46",
    },
    dark: {
      accent: "#8dead9",
      hover: "#b0f3e6",
      onAccent: "#222222",
      label: "#8dead9",
    },
  },
  blue: {
    light: {
      accent: "#4594fb",
      hover: "#3287ee",
      onAccent: "#ffffff",
      label: "#4594fb",
    },
    dark: {
      accent: "#9bc7ff",
      hover: "#bddaff",
      onAccent: "#222222",
      label: "#9bc7ff",
    },
  },
  purple: {
    light: {
      accent: "#8b3dff",
      hover: "#7625ed",
      onAccent: "#ffffff",
      label: "#6524c8",
    },
    dark: {
      accent: "#d0b8ff",
      hover: "#dfcfff",
      onAccent: "#222222",
      label: "#d0b8ff",
    },
  },
  pink: {
    light: {
      accent: "#de1168",
      hover: "#cc085a",
      onAccent: "#ffffff",
      label: "#a80a49",
    },
    dark: {
      accent: "#ffafd1",
      hover: "#ffcee4",
      onAccent: "#222222",
      label: "#ffafd1",
    },
  },
};

export function loadAppearancePreferences(
  storage: PreferenceStorage | undefined,
  defaultMode: AppearanceMode,
): AppearancePreferences {
  const fallback: AppearancePreferences = {
    mode: defaultMode,
    color: defaultMode === "light" ? "blue" : "yellow",
  };
  try {
    const raw: unknown = JSON.parse(
      storage?.getItem(appearanceStorageKey) ?? "null",
    );
    if (!raw || typeof raw !== "object") return fallback;
    const value = raw as Record<string, unknown>;
    return {
      mode:
        value.mode === "light" || value.mode === "dark"
          ? value.mode
          : defaultMode,
      color: appearanceColors.includes(value.color as AppearanceColor)
        ? (value.color as AppearanceColor)
        : fallback.color,
    };
  } catch {
    return fallback;
  }
}

export function saveAppearancePreferences(
  storage: PreferenceStorage | undefined,
  preferences: AppearancePreferences,
): boolean {
  try {
    if (!storage) return false;
    storage.setItem(appearanceStorageKey, JSON.stringify(preferences));
    return true;
  } catch {
    return false;
  }
}

function browserStorage(): PreferenceStorage | undefined {
  try {
    return window.localStorage;
  } catch {
    return undefined;
  }
}

export function applyAppearance(preferences: AppearancePreferences): void {
  const root = document.documentElement;
  const palette = appearancePalettes[preferences.color][preferences.mode];
  root.dataset.previewTheme = preferences.mode;
  root.dataset.appearanceColor = preferences.color;
  root.style.setProperty("--accent", palette.accent);
  root.style.setProperty("--accent-hover", palette.hover);
  root.style.setProperty("--on-accent", palette.onAccent);
  root.style.setProperty("--accent-text", palette.label);
  root.style.setProperty("--focus", palette.label);
  for (const meta of document.querySelectorAll<HTMLMetaElement>(
    'meta[name="theme-color"]',
  )) {
    meta.content = preferences.mode === "dark" ? "#222222" : "#f4f5f7";
    meta.removeAttribute("media");
  }
}

export function initializeAppearance(designPreview: boolean): void {
  const queryMode = new URLSearchParams(location.search).get("theme");
  const defaultMode: AppearanceMode =
    queryMode === "light" || queryMode === "dark"
      ? queryMode
      : designPreview ||
          window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
  applyAppearance(loadAppearancePreferences(browserStorage(), defaultMode));
}

export function currentAppearance(): AppearancePreferences {
  if (typeof document === "undefined") return { mode: "dark", color: "yellow" };
  const { previewTheme, appearanceColor } = document.documentElement.dataset;
  return {
    mode: previewTheme === "light" ? "light" : "dark",
    color: appearanceColors.includes(appearanceColor as AppearanceColor)
      ? (appearanceColor as AppearanceColor)
      : "yellow",
  };
}

export function persistAppearance(preferences: AppearancePreferences): boolean {
  return saveAppearancePreferences(browserStorage(), preferences);
}
