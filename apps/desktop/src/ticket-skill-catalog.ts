import { invoke, isTauri } from "@tauri-apps/api/core";
import type { SkillCandidate } from "./ticket-prompt";

export type TicketSkillCatalog = {
  skills: SkillCandidate[];
  source: "mac" | "snapshot" | "unavailable";
};

export function validSkillCatalog(
  value: unknown,
): SkillCandidate[] | undefined {
  if (!Array.isArray(value)) return;
  const skills = new Map<string, string>();
  for (const entry of value) {
    if (
      !entry ||
      typeof entry !== "object" ||
      typeof entry.name !== "string" ||
      !/^[a-z0-9_:.-]{1,96}$/i.test(entry.name)
    )
      continue;
    const description =
      typeof entry.description === "string"
        ? Array.from(entry.description.trim().replace(/\s+/g, " "))
            .slice(0, 260)
            .join("")
        : "";
    skills.set(
      entry.name,
      !skills.has(entry.name) || skills.get(entry.name) === description
        ? description
        : "",
    );
    if (skills.size === 256) break;
  }
  return [...skills].map(([name, description]) => ({ name, description }));
}

export async function readTicketSkillCatalog(): Promise<TicketSkillCatalog> {
  if (isTauri() && !/Android/i.test(navigator.userAgent)) {
    try {
      const skills = validSkillCatalog(await invoke("list_local_codex_skills"));
      if (skills) return { skills, source: "mac" };
    } catch {
      // A missing CLI does not prevent generating a plain draft.
    }
  }
  try {
    const skills = validSkillCatalog(
      JSON.parse(import.meta.env.VITE_CODEX_SKILL_CATALOG ?? "null"),
    );
    if (skills) return { skills, source: "snapshot" };
  } catch {
    // Direct browser and local development builds may not contain a catalog.
  }
  return { skills: [], source: "unavailable" };
}
