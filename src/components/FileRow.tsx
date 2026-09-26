import { memo } from "react";
import { formatBytes, formatDate } from "../lib/format";
import type { DownloadRecord } from "../types";
import { DownloadActions } from "./DownloadActions";
import { Icon } from "./Icon";
import { StatusBadge } from "./StatusBadge";

function IntegrityLabel({ record }: { record: DownloadRecord }) {
  if (record.integrity === "verified") {
    return (
      <span className="integrity ok" title={`${record.checksum?.algorithm.toUpperCase()} checksum matched`}>
        <Icon name="shield" size={15} /> Verified
      </span>
    );
  }
  if (record.integrity === "failed") {
    return (
      <span className="integrity bad">
        <Icon name="shieldX" size={15} /> Verification failed
      </span>
    );
  }
  return <span className="integrity muted">Not verified</span>;
}

/** A compact row for finished downloads (Completed and History pages). */
export const FileRow = memo(function FileRow({ record, showStatus }: { record: DownloadRecord; showStatus?: boolean }) {
  const size = record.totalSize ?? (record.downloaded || null);
  const location = record.finalPath ?? record.root;
  // Show the name actually used on disk, which may carry a " (1)" suffix.
  const name = record.finalPath?.split(/[\\/]/).pop() || record.fileName;
  return (
    <li className="file-row" data-status={record.status}>
      <div className="file-icon">
        <Icon name="file" size={19} />
      </div>
      <div className="file-main">
        <p className="file-name" title={name}>
          {name}
        </p>
        <p className="muted small file-meta">
          <span>{size === null ? "Unknown size" : formatBytes(size)}</span>
          <span>{formatDate(record.completedAt ?? record.createdAt)}</span>
          {record.status === "completed" && <IntegrityLabel record={record} />}
          {record.status === "failed" && record.error && (
            <span className="text-bad" title={record.error.details ?? undefined}>
              {record.error.message.split("\n")[0]}
            </span>
          )}
        </p>
        <p className="muted small mono file-location" title={location}>
          {location}
        </p>
      </div>
      {showStatus && <StatusBadge status={record.status} />}
      <DownloadActions record={record} />
    </li>
  );
});
