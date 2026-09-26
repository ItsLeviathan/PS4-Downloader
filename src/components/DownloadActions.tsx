import { useState } from "react";
import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import { runAction } from "../stores/app";
import type { DownloadRecord } from "../types";
import { Button } from "./Button";
import { ConfirmDialog } from "./ConfirmDialog";

type Dialog = "cancel" | "remove" | null;

/** The buttons that apply to a download in its current state. */
export function DownloadActions({ record }: { record: DownloadRecord }) {
  const [dialog, setDialog] = useState<Dialog>(null);
  const [busy, setBusy] = useState(false);
  const { id, status } = record;

  const act = async (action: () => Promise<unknown>) => {
    setBusy(true);
    await runAction(action);
    setBusy(false);
  };

  const hasPartial = record.downloaded > 0 && status !== "completed";

  return (
    <div className="actions">
      {(status === "downloading" || status === "retrying" || status === "queued") && (
        <Button size="sm" icon="pause" disabled={busy} onClick={() => act(() => api.pause(id))}>
          Pause
        </Button>
      )}
      {status === "paused" && (
        <Button size="sm" variant="primary" icon="play" disabled={busy} onClick={() => act(() => api.resume(id))}>
          Resume
        </Button>
      )}
      {(status === "failed" || status === "cancelled") && (
        <Button size="sm" variant="primary" icon="retry" disabled={busy} onClick={() => act(() => api.retry(id))}>
          Retry
        </Button>
      )}
      {status === "completed" && (
        <Button size="sm" icon="folder" onClick={() => act(() => api.reveal(id))}>
          Open folder
        </Button>
      )}
      {(status === "downloading" || status === "retrying" || status === "queued" || status === "paused") && (
        <Button size="sm" variant="ghost" icon="x" disabled={busy} onClick={() => setDialog("cancel")}>
          Cancel
        </Button>
      )}
      {(status === "failed" || status === "cancelled" || status === "completed") && (
        <Button size="sm" variant="ghost" icon="trash" disabled={busy} onClick={() => setDialog("remove")}>
          Remove
        </Button>
      )}

      {dialog === "cancel" && (
        <ConfirmDialog
          title="Cancel this download?"
          confirmLabel="Cancel download"
          danger
          onConfirm={() => act(() => api.cancel(id))}
          onClose={() => setDialog(null)}
        >
          <p>
            <strong>{record.fileName}</strong> will stop downloading
            {hasPartial ? ` and the ${formatBytes(record.downloaded)} downloaded so far will be deleted.` : "."} You can
            start it again later from History.
          </p>
        </ConfirmDialog>
      )}
      {dialog === "remove" && (
        <ConfirmDialog
          title="Remove from list?"
          confirmLabel="Remove"
          danger={status !== "completed"}
          option={status === "completed" ? "Also delete the file from disk" : undefined}
          onConfirm={(deleteFile) => act(() => api.remove(id, deleteFile))}
          onClose={() => setDialog(null)}
        >
          {status === "completed" ? (
            <p>
              <strong>{record.fileName}</strong> will be removed from History. The file stays in the Completed folder
              unless you choose to delete it.
            </p>
          ) : (
            <p>
              <strong>{record.fileName}</strong> will be removed from History
              {hasPartial ? ` and its partial data (${formatBytes(record.downloaded)}) will be deleted.` : "."}
            </p>
          )}
        </ConfirmDialog>
      )}
    </div>
  );
}
