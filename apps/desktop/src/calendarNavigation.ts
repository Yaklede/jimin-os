export type CalendarNavigationIntent = {
  planningReady: boolean;
  targetStartsAt?: string;
};

export type CalendarDestinationLoad = {
  shouldLoadPlanning: boolean;
  targetStartsAt?: string;
};

export type CalendarDestinationActivation = {
  active: boolean;
  shouldLoad: boolean;
};

export function calendarDestinationActivation(
  wasActive: boolean,
  isCalendarDestination: boolean,
): CalendarDestinationActivation {
  if (!isCalendarDestination) {
    return { active: false, shouldLoad: false };
  }
  return { active: true, shouldLoad: !wasActive };
}

export function calendarDestinationLoad(
  intent?: CalendarNavigationIntent,
): CalendarDestinationLoad {
  if (intent?.planningReady) {
    return { shouldLoadPlanning: false };
  }
  return {
    shouldLoadPlanning: true,
    targetStartsAt: intent?.targetStartsAt,
  };
}
