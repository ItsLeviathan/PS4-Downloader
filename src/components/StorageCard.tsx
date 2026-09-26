import { useStorageActions } from "../hooks/useStorageActions";
import { formatBytes } from "../lib/format";
import { useApp } from "../stores/app";
import type { StorageInfo } from "../types";
import { Button } from "./Button";
import { Icon } from "./Icon";

/** Explains why storage is unusable and offers the fix. Renders nothing when storage is fine. */
export function StorageWarning({ storage }: { storage: StorageInfo }) {
  const { chooseFolder, createFolder } = useStorageActions();
  if (storage.available) return null;
  return (
    <div className="notice notice-warning storage-warning" role="alert">
      <Icon name="alert" size={17} className="notice-icon" />
      <div className="notice-body">
        <p className="notice-message">
          {storage.driveAvailable
            ? `The folder ${storage.root} doesn't exist yet.`
            : `Storage drive unavailable. ${storage.root} can't be reached — reconnect the drive or choose another folder.`}
        </p>
        <div className="actions">
          {storage.driveAvailable && (
            <Button size="sm" variant="primary" onClick={createFolder}>
              Create folder
            </Button>
          )}
          <Button size="sm" icon="folder" onClick={chooseFolder}>
            Choose another folder
          </Button>
        </div>
      </div>
    </div>
  );
}

export function StorageCard() {
  const storage = useApp((s) => s.storage);
  if (!storage) return null;
  const usedPercent = storage.total > 0 ? (storage.used / storage.total) * 100 : 0;
  const tight = storage.available && storage.total > 0 && storage.free / storage.total < 0.1;

  return (
    <section className="card storage-card" aria-label="Storage">
      <div className="card-head">
        <div className="card-title">
          <Icon name="drive" size={18} />
          <h2>Storage</h2>
        </div>
        <span className={`dot-label ${storage.available ? "ok" : "bad"}`}>
          {storage.available ? "Connected" : "Unavailable"}
        </span>
      </div>
      <p className="storage-path mono" title={storage.root}>
        {storage.root}
      </p>

      {storage.available ? (
        <>
          <div
            className={`meter${tight ? " meter-tight" : ""}`}
            role="meter"
            aria-label="Drive usage"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(usedPercent)}
          >
            <div className="meter-fill" style={{ width: `${usedPercent}%` }} />
          </div>
          <dl className="storage-stats">
            <div>
              <dt>Free</dt>
              <dd>{formatBytes(storage.free)}</dd>
            </div>
            <div>
              <dt>Used</dt>
              <dd>{formatBytes(storage.used)}</dd>
            </div>
            <div>
              <dt>Total</dt>
              <dd>{formatBytes(storage.total)}</dd>
            </div>
          </dl>
        </>
      ) : (
        <StorageWarning storage={storage} />
      )}
    </section>
  );
}
