import { useState, type FormEvent } from "react";
import { api } from "../lib/api";
import { runAction, useApp } from "../stores/app";
import { clearGameDetails } from "../stores/games";
import { Button } from "./Button";

export const RAWG_KEY_PAGE = "https://rawg.io/apidocs";

/** Input for the RAWG API key. Saves into Settings. */
export function ApiKeyForm({ onSaved }: { onSaved?: () => void }) {
  const settings = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const [key, setKey] = useState(settings?.rawgApiKey ?? "");
  const [busy, setBusy] = useState(false);
  if (!settings) return null;

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setBusy(true);
    const ok = await runAction(async () => setSettings(await api.updateSettings({ ...settings, rawgApiKey: key.trim() })));
    setBusy(false);
    if (ok) {
      // Game pages opened before now were built without (or with the old) key.
      clearGameDetails();
      onSaved?.();
    }
  };

  const changed = key.trim() !== settings.rawgApiKey;

  return (
    <form className="key-form" onSubmit={submit}>
      <input
        className="input mono"
        type="password"
        spellCheck={false}
        autoComplete="off"
        placeholder="RAWG API key"
        aria-label="RAWG API key"
        value={key}
        onChange={(e) => setKey(e.target.value)}
      />
      <Button type="submit" variant="primary" busy={busy} disabled={!changed}>
        Save key
      </Button>
    </form>
  );
}

export function GetKeyLink() {
  return (
    <button type="button" className="link-button" onClick={() => runAction(() => api.openExternal(RAWG_KEY_PAGE))}>
      Get a free key at rawg.io
    </button>
  );
}
