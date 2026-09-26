import { useMemo } from "react";
import { useShallow } from "zustand/react/shallow";
import { useApp } from "../stores/app";
import type { DownloadRecord, DownloadStatus, ProgressItem } from "../types";

export const ACTIVE_STATUSES: DownloadStatus[] = ["downloading", "retrying", "verifying", "queued"];
export const FINISHED_STATUSES: DownloadStatus[] = ["completed", "failed", "cancelled"];

/**
 * Downloads whose status is in `statuses`, preserving queue order.
 * Pass a module-level constant so the memo stays stable.
 */
export function useDownloadsByStatus(statuses: readonly DownloadStatus[]): DownloadRecord[] {
  const downloads = useApp((s) => s.downloads);
  return useMemo(() => downloads.filter((d) => statuses.includes(d.status)), [downloads, statuses]);
}

export function useProgress(id: string): ProgressItem | undefined {
  return useApp((s) => s.progress[id]);
}

export function useStatusCounts() {
  return useApp(
    useShallow((s) => {
      let active = 0;
      let paused = 0;
      let failed = 0;
      for (const d of s.downloads) {
        if (d.status === "paused") paused++;
        else if (d.status === "failed") failed++;
        else if (ACTIVE_STATUSES.includes(d.status)) active++;
      }
      return { active, paused, failed };
    }),
  );
}

/** Combined transfer speed of all running downloads. */
export function useTotalSpeed(): number {
  return useApp((s) => Object.values(s.progress).reduce((sum, p) => sum + p.speed, 0));
}
