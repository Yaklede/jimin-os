import type { ProjectInflowItem } from "./api/googleChat";

export function inflowReviewSnapshotKey(item: ProjectInflowItem): string {
  return JSON.stringify([
    item.projectId,
    item.id,
    item.version,
    item.sourceRevision,
  ]);
}

export async function markInflowSeenBatch(
  items: readonly ProjectInflowItem[],
  markSeen: (item: ProjectInflowItem) => Promise<ProjectInflowItem>,
) {
  const pending = [
    ...new Map(
      items.map((item) => [inflowReviewSnapshotKey(item), item]),
    ).values(),
  ];
  const updatedVersions = new Map<string, number>();
  const failed: ProjectInflowItem[] = [];
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(3, pending.length) }, async () => {
      while (next < pending.length) {
        const item = pending[next++];
        try {
          const updated = await markSeen(item);
          updatedVersions.set(inflowReviewSnapshotKey(item), updated.version);
        } catch {
          failed.push(item);
        }
      }
    }),
  );
  return { updatedVersions, failed };
}

export type InflowBatchReviewResult = Awaited<
  ReturnType<typeof markInflowSeenBatch>
>;

export function reconcileInflowSeenBatch(
  items: ProjectInflowItem[],
  updatedVersions: ReadonlyMap<string, number>,
): ProjectInflowItem[] {
  return items.map((item) => {
    const version = updatedVersions.get(inflowReviewSnapshotKey(item));
    return version === undefined ? item : { ...item, reviewed: true, version };
  });
}
