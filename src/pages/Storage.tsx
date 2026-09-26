import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { PageHeader } from "../components/Layout";
import { StorageCard } from "../components/StorageCard";
import { useStorageActions } from "../hooks/useStorageActions";
import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import { runAction, useApp } from "../stores/app";
import type { StorageFolder, StorageUsage } from "../types";

const FOLDERS: { folder: StorageFolder; name: string; description: string }[] = [
  { folder: "completed", name: "Completed", description: "Finished files." },
  { folder: "incomplete", name: "Incomplete", description: "Paused downloads that can be resumed." },
  { folder: "root", name: "Storage folder", description: "Contains Downloads, Completed, Incomplete and Metadata." },
];

export function Storage() {
  const storage = useApp((s) => s.storage);
  const downloads = useApp((s) => s.downloads);
  const { chooseFolder } = useStorageActions();
  const [usage, setUsage] = useState<StorageUsage | null>(null);

  // Recount when the location or the set of downloads changes; not on every progress tick.
  const finishedCount = downloads.filter((d) => d.status === "completed").length;
  useEffect(() => {
    if (!storage?.available) {
      setUsage(null);
      return;
    }
    let cancelled = false;
    api
      .getStorageUsage()
      .then((u) => !cancelled && setUsage(u))
      .catch(() => !cancelled && setUsage(null));
    return () => {
      cancelled = true;
    };
  }, [storage?.root, storage?.available, finishedCount, downloads.length]);

  return (
    <div className="page">
      <PageHeader
        title="Storage"
        subtitle="Where downloads are written and how much room is left."
        actions={
          <Button icon="folder" onClick={chooseFolder}>
            Change location
          </Button>
        }
      />
      <StorageCard />

      {storage?.available && (
        <div className="usage-grid">
          <div className="card usage-card">
            <span className="muted small">Completed files</span>
            <span className="summary-value">{usage ? formatBytes(usage.completed.bytes) : "…"}</span>
            <span className="muted small">{usage ? `${usage.completed.files} files` : " "}</span>
          </div>
          <div className="card usage-card">
            <span className="muted small">Partial downloads</span>
            <span className="summary-value">{usage ? formatBytes(usage.partial.bytes) : "…"}</span>
            <span className="muted small">{usage ? `${usage.partial.files} files` : " "}</span>
          </div>
        </div>
      )}

      {storage?.available && (
        <ul className="card folder-list">
          {FOLDERS.map((f) => (
            <li key={f.folder} className="folder-row">
              <div>
                <p className="file-name">{f.name}</p>
                <p className="muted small">{f.description}</p>
              </div>
              <Button size="sm" icon="folder" onClick={() => runAction(() => api.openFolder(f.folder))}>
                Open
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
