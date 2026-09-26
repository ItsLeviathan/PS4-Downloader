import { useState, type FormEvent } from "react";
import { api, toAppError } from "../lib/api";
import type { AppError } from "../types";
import { Button } from "./Button";
import { ErrorNotice } from "./ErrorNotice";
import { Icon } from "./Icon";

/** URL input. Nothing is contacted until the user presses Download. */
export function AddDownloadForm() {
  const [url, setUrl] = useState("");
  const [checksum, setChecksum] = useState("");
  const [showChecksum, setShowChecksum] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [added, setAdded] = useState<string | null>(null);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    if (!url.trim() || busy) return;
    setBusy(true);
    setError(null);
    setAdded(null);
    try {
      const record = await api.addDownload(url, showChecksum ? checksum : undefined);
      setAdded(record.fileName);
      setUrl("");
      setChecksum("");
    } catch (err) {
      setError(toAppError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="card add-form" onSubmit={submit}>
      <div className="add-row">
        <label className="url-field">
          <Icon name="link" size={18} className="url-icon" />
          <input
            type="text"
            inputMode="url"
            spellCheck={false}
            autoComplete="off"
            placeholder="Paste download URL"
            aria-label="Download URL"
            value={url}
            onChange={(e) => {
              setUrl(e.target.value);
              setError(null);
              setAdded(null);
            }}
          />
        </label>
        <Button type="submit" variant="primary" icon="download" busy={busy} disabled={!url.trim()}>
          {busy ? "Checking link…" : "Download"}
        </Button>
      </div>

      <div className="add-extra">
        <button type="button" className="link-button" onClick={() => setShowChecksum((s) => !s)} aria-expanded={showChecksum}>
          {showChecksum ? "Remove checksum" : "Add checksum (optional)"}
        </button>
        {added && (
          <span className="add-ok">
            <Icon name="check" size={15} /> Added {added}
          </span>
        )}
      </div>

      {showChecksum && (
        <input
          className="input mono"
          type="text"
          spellCheck={false}
          autoComplete="off"
          placeholder="SHA-256, SHA-1 or MD5 from the download page, e.g. sha256:9f86d0…"
          aria-label="Expected checksum"
          value={checksum}
          onChange={(e) => setChecksum(e.target.value)}
        />
      )}

      {error && <ErrorNotice error={error} />}
    </form>
  );
}
