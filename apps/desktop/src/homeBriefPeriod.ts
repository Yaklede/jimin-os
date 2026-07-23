export type HomeBriefPeriod = "morning" | "daytime" | "evening";

export function homeBriefPeriodForHour(hour: number): HomeBriefPeriod {
  if (hour < 12) return "morning";
  if (hour < 18) return "daytime";
  return "evening";
}
