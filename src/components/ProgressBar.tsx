import type { DownloadStatus } from "../types";

interface ProgressBarProps {
  /** 0–100, or null when the total size is unknown. */
  value: number | null;
  status?: DownloadStatus;
  label?: string;
}

export function ProgressBar({ value, status, label = "Download progress" }: ProgressBarProps) {
  const indeterminate = value === null && (status === "downloading" || status === "verifying");
  return (
    <div
      className={`progress${indeterminate ? " progress-indeterminate" : ""}`}
      data-status={status}
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value ?? undefined}
    >
      <div className="progress-fill" style={indeterminate ? undefined : { width: `${value ?? 0}%` }} />
    </div>
  );
}
