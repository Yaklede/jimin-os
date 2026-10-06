import type { ScheduleEntry } from "./api/planning";
import { scheduleOverlapsPlanningDay } from "./planningRange";

export interface WeekTimedItem {
  id: string;
  title: string;
  kind: "schedule" | "report";
  startsAt: string;
  endsAt?: string;
  entry?: ScheduleEntry;
}
export interface WeekTimedBlock extends WeekTimedItem {
  startMinute: number;
  endMinute: number;
  column: number;
  columns: number;
}

export function isAllDaySchedule(entry: ScheduleEntry): boolean {
  const start = new Date(entry.startsAt);
  const end = new Date(entry.endsAt);
  return (
    end > start &&
    start.getHours() === 0 &&
    start.getMinutes() === 0 &&
    start.getSeconds() === 0 &&
    start.getMilliseconds() === 0 &&
    end.getHours() === 0 &&
    end.getMinutes() === 0 &&
    end.getSeconds() === 0 &&
    end.getMilliseconds() === 0
  );
}

// Position by local clock minutes, matching the calendar's local date boundaries.
export function layoutWeekDay(
  items: WeekTimedItem[],
  day: Date,
): WeekTimedBlock[] {
  const next = new Date(day.getFullYear(), day.getMonth(), day.getDate() + 1);
  const blocks: WeekTimedBlock[] = [];
  for (const item of items) {
    const start = new Date(item.startsAt);
    const end = item.endsAt
      ? new Date(item.endsAt)
      : new Date(start.getTime() + 1);
    if (
      !Number.isFinite(start.getTime()) ||
      !Number.isFinite(end.getTime()) ||
      end <= start
    )
      continue;
    if (
      !scheduleOverlapsPlanningDay(
        { startsAt: item.startsAt, endsAt: end.toISOString() },
        day,
      )
    )
      continue;
    const startMinute =
      start <= day ? 0 : start.getHours() * 60 + start.getMinutes();
    const endMinute =
      end >= next ? 1440 : end.getHours() * 60 + end.getMinutes();
    blocks.push({ ...item, startMinute, endMinute, column: 0, columns: 1 });
  }
  blocks.sort(
    (a, b) =>
      a.startMinute - b.startMinute ||
      b.endMinute - a.endMinute ||
      a.id.localeCompare(b.id),
  );
  let cluster: WeekTimedBlock[] = [];
  let laneEnds: number[] = [];
  let clusterEnd = 0;
  const finish = () => {
    for (const block of cluster) block.columns = laneEnds.length;
  };
  for (const block of blocks) {
    // Short entries and received requests still need a readable touch target.
    const visualEnd = Math.min(
      1440,
      Math.max(block.endMinute, block.startMinute + 45),
    );
    if (cluster.length && block.startMinute >= clusterEnd) {
      finish();
      cluster = [];
      laneEnds = [];
    }
    let lane = laneEnds.findIndex((end) => end <= block.startMinute);
    if (lane < 0) lane = laneEnds.length;
    laneEnds[lane] = visualEnd;
    block.column = lane;
    cluster.push(block);
    clusterEnd = Math.max(...laneEnds);
  }
  finish();
  return blocks;
}

export function weekTimeWindow(blocks: WeekTimedBlock[]): {
  startHour: number;
  endHour: number;
} {
  return {
    startHour: Math.min(
      7,
      ...blocks.map((block) => Math.floor(block.startMinute / 60)),
    ),
    endHour: Math.min(
      24,
      Math.max(
        20,
        ...blocks.map((block) =>
          Math.ceil(Math.max(block.endMinute, block.startMinute + 45) / 60),
        ),
      ),
    ),
  };
}
