import type { DownloadStatus } from "../types";

const LABELS: Record<DownloadStatus, string> = {
  queued: "Waiting",
  downloading: "Downloading",
  retrying: "Reconnecting",
  verifying: "Verifying",
  paused: "Paused",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
};

export function StatusBadge({ status }: { status: DownloadStatus }) {
  return (
    <span className="badge" data-status={status}>
      <span className="badge-dot" aria-hidden="true" />
      {LABELS[status]}
    </span>
  );
}
