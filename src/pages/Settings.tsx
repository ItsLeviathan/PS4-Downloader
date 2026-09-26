import type { ReactNode } from "react";
import { Button } from "../components/Button";
import { PageHeader } from "../components/Layout";
import { useStorageActions } from "../hooks/useStorageActions";
import { api } from "../lib/api";
import { runAction, useApp } from "../stores/app";
import type { Settings as SettingsType, Theme } from "../types";

const CONNECTIONS = [1, 2, 4, 8];
const SIMULTANEOUS = [1, 2, 3, 4, 5];
const THEMES: { value: Theme; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
];

function Row({ label, description, children }: { label: string; description?: string; children: ReactNode }) {
  return (
    <div className="setting-row">
      <div className="setting-text">
        <p className="setting-label">{label}</p>
        {description && <p className="muted small">{description}</p>}
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button type="button" role="switch" aria-checked={checked} aria-label={label} className="switch" onClick={() => onChange(!checked)}>
      <span className="switch-thumb" />
    </button>
  );
}

function Choice<T extends string | number>({ options, value, onChange, label }: { options: { value: T; label: string }[]; value: T; onChange: (v: T) => void; label: string }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" role="radio" aria-checked={o.value === value} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

export function Settings() {
  const settings = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const { chooseFolder } = useStorageActions();
  if (!settings) return null;

  const save = (patch: Partial<SettingsType>) =>
    runAction(async () => setSettings(await api.updateSettings({ ...settings, ...patch })));

  return (
    <div className="page">
      <PageHeader title="Settings" subtitle="Changes are saved immediately." />

      <section className="card settings-group">
        <h2>Storage</h2>
        <Row label="Download location" description="New downloads are saved here. Existing downloads stay where they are.">
          <span className="mono setting-path" title={settings.storageRoot}>
            {settings.storageRoot}
          </span>
          <Button size="sm" icon="folder" onClick={chooseFolder}>
            Change
          </Button>
        </Row>
      </section>

      <section className="card settings-group">
        <h2>Downloads</h2>
        <Row label="Simultaneous downloads" description="Additional downloads wait in the queue.">
          <Choice
            label="Simultaneous downloads"
            options={SIMULTANEOUS.map((n) => ({ value: n, label: String(n) }))}
            value={settings.maxSimultaneous}
            onChange={(maxSimultaneous) => save({ maxSimultaneous })}
          />
        </Row>
        <Row
          label="Connections per download"
          description="Used only when the server supports resuming. More connections aren't always faster; applies to new downloads."
        >
          <Choice
            label="Connections per download"
            options={CONNECTIONS.map((n) => ({ value: n, label: String(n) }))}
            value={settings.connections}
            onChange={(connections) => save({ connections })}
          />
        </Row>
        <Row label="Start downloads automatically" description="Resume interrupted and queued downloads when PS4 Downloader opens.">
          <Toggle label="Start downloads automatically" checked={settings.autoStart} onChange={(autoStart) => save({ autoStart })} />
        </Row>
      </section>

      <section className="card settings-group">
        <h2>Notifications</h2>
        <Row label="Download completed">
          <Toggle label="Download completed" checked={settings.notifyCompleted} onChange={(notifyCompleted) => save({ notifyCompleted })} />
        </Row>
        <Row label="Download failed">
          <Toggle label="Download failed" checked={settings.notifyFailed} onChange={(notifyFailed) => save({ notifyFailed })} />
        </Row>
        <Row label="Storage warnings" description="For example when the drive is disconnected during a download.">
          <Toggle label="Storage warnings" checked={settings.notifyStorage} onChange={(notifyStorage) => save({ notifyStorage })} />
        </Row>
      </section>

      <section className="card settings-group">
        <h2>Appearance</h2>
        <Row label="Theme">
          <Choice label="Theme" options={THEMES} value={settings.theme} onChange={(theme) => save({ theme })} />
        </Row>
      </section>
    </div>
  );
}
