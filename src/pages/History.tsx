import { useMemo, useState } from "react";
import { FileRow } from "../components/FileRow";
import { EmptyState, PageHeader } from "../components/Layout";
import { FINISHED_STATUSES, useDownloadsByStatus } from "../hooks/useDownloads";
import type { DownloadStatus } from "../types";

type Filter = "all" | Extract<DownloadStatus, "completed" | "failed" | "cancelled">;

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "completed", label: "Completed" },
  { value: "failed", label: "Failed" },
  { value: "cancelled", label: "Cancelled" },
];

export function History() {
  const finished = useDownloadsByStatus(FINISHED_STATUSES);
  const [filter, setFilter] = useState<Filter>("all");

  const rows = useMemo(
    () =>
      finished
        .filter((d) => filter === "all" || d.status === filter)
        .sort((a, b) => (b.completedAt ?? b.createdAt) - (a.completedAt ?? a.createdAt)),
    [finished, filter],
  );

  return (
    <div className="page">
      <PageHeader title="History" subtitle="Removing an entry never deletes a downloaded file unless you ask it to." />
      <div className="segmented" role="tablist" aria-label="Filter history">
        {FILTERS.map((f) => (
          <button
            key={f.value}
            type="button"
            role="tab"
            aria-selected={filter === f.value}
            onClick={() => setFilter(f.value)}
          >
            {f.label}
            <span className="segmented-count">
              {f.value === "all" ? finished.length : finished.filter((d) => d.status === f.value).length}
            </span>
          </button>
        ))}
      </div>
      {rows.length === 0 ? (
        <EmptyState icon="history" title="Nothing here yet">
          Completed, failed and cancelled downloads are listed here.
        </EmptyState>
      ) : (
        <ul className="card file-list">
          {rows.map((d) => (
            <FileRow key={d.id} record={d} showStatus />
          ))}
        </ul>
      )}
    </div>
  );
}
