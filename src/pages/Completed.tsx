import { useMemo } from "react";
import { Button } from "../components/Button";
import { FileRow } from "../components/FileRow";
import { EmptyState, PageHeader } from "../components/Layout";
import { useDownloadsByStatus } from "../hooks/useDownloads";
import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import { runAction } from "../stores/app";
import type { DownloadStatus } from "../types";

const COMPLETED: DownloadStatus[] = ["completed"];

export function Completed() {
  const completed = useDownloadsByStatus(COMPLETED);
  const sorted = useMemo(
    () => [...completed].sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0)),
    [completed],
  );
  const totalBytes = completed.reduce((sum, d) => sum + (d.totalSize ?? 0), 0);

  return (
    <div className="page">
      <PageHeader
        title="Completed"
        subtitle={completed.length ? `${completed.length} files · ${formatBytes(totalBytes)}` : undefined}
        actions={
          <Button icon="folder" onClick={() => runAction(() => api.openFolder("completed"))}>
            Open Completed folder
          </Button>
        }
      />
      {sorted.length === 0 ? (
        <EmptyState icon="check" title="No completed downloads yet">
          Finished files are moved to the Completed folder and listed here.
        </EmptyState>
      ) : (
        <ul className="card file-list">
          {sorted.map((d) => (
            <FileRow key={d.id} record={d} />
          ))}
        </ul>
      )}
    </div>
  );
}
