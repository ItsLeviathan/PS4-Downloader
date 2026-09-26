import { memo } from "react";
import { useProgress } from "../hooks/useDownloads";
import { useNow } from "../hooks/useNow";
import { formatBytes, formatEta, formatPercent, formatSpeed } from "../lib/format";
import type { DownloadRecord } from "../types";
import { DownloadActions } from "./DownloadActions";
import { ErrorNotice } from "./ErrorNotice";
import { Icon } from "./Icon";
import { ProgressBar } from "./ProgressBar";
import { StatusBadge } from "./StatusBadge";

function hostOf(url: string): string {
  try {
    return new URL(url).host;
  } catch {
    return url;
  }
}

export const DownloadCard = memo(function DownloadCard({ record, position }: { record: DownloadRecord; position?: number }) {
  const live = useProgress(record.id);
  const now = useNow(record.status === "retrying");
  const downloaded = live?.downloaded ?? record.downloaded;
  const total = live?.total ?? record.totalSize;
  const percent = formatPercent(downloaded, total);
  const transferring = record.status === "downloading";
  const retryIn = record.retry ? Math.max(0, Math.ceil((record.retry.retryAt - now) / 1000)) : 0;
  const showError = record.error && record.status !== "retrying";

  return (
    <article className="card dl-card" data-status={record.status}>
      <div className="dl-head">
        <div className="file-icon">
          {position !== undefined ? <span className="queue-pos">{position}</span> : <Icon name="file" size={20} />}
        </div>
        <div className="dl-title">
          <h3 title={record.fileName}>{record.fileName}</h3>
          <span className="muted small" title={record.url}>
            {hostOf(record.url)}
          </span>
        </div>
        <StatusBadge status={record.status} />
      </div>

      <ProgressBar value={percent} status={record.status} />

      <div className="dl-foot">
        <div className="dl-stats">
          <span className="dl-percent">{percent === null ? "—" : `${percent}%`}</span>
          <span className="dl-stat">
            {formatBytes(downloaded)} / {total === null ? "Unknown size" : formatBytes(total)}
          </span>
          {transferring && live && (
            <>
              <span className="dl-stat dl-speed">{formatSpeed(live.speed)}</span>
              <span className="dl-stat">ETA {formatEta(live.eta)}</span>
            </>
          )}
          {record.status === "verifying" && <span className="dl-stat">Checking file integrity…</span>}
          {record.connections > 1 && record.status !== "completed" && (
            <span className="dl-stat muted">{record.connections} connections</span>
          )}
        </div>
        <DownloadActions record={record} />
      </div>

      {record.status === "retrying" && record.retry && (
        <p className="dl-retry">
          <Icon name="retry" size={15} />
          {record.error?.message.split("\n")[0] ?? "Connection lost."} Retrying in {retryIn}s… Attempt{" "}
          {record.retry.attempt}/{record.retry.maxAttempts}
        </p>
      )}
      {showError && record.error && <ErrorNotice error={record.error} tone={record.status === "paused" ? "warning" : "error"} />}
    </article>
  );
});
