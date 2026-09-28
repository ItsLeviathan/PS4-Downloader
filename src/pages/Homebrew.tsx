import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { ErrorNotice } from "../components/ErrorNotice";
import { Icon } from "../components/Icon";
import { EmptyState, PageHeader } from "../components/Layout";
import { api } from "../lib/api";
import { formatBytes, formatReleaseDate } from "../lib/format";
import { runAction } from "../stores/app";
import { useHomebrew } from "../stores/homebrew";
import type { HomebrewApp, HomebrewAsset, HomebrewCategory, Page } from "../types";

type Filter = HomebrewCategory | "all";

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "All" },
  { value: "essentials", label: "Essentials" },
  { value: "tools", label: "Tools" },
  { value: "payloads", label: "Payloads" },
];

const CATEGORY_LABELS: Record<HomebrewCategory, string> = {
  essentials: "Essentials",
  tools: "Tool",
  payloads: "Payload",
};

export function Homebrew({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const { apps, loading, error, load } = useHomebrew();
  const [filter, setFilter] = useState<Filter>("all");

  useEffect(() => {
    if (useHomebrew.getState().apps.length === 0) load();
  }, [load]);

  const shown = filter === "all" ? apps : apps.filter((a) => a.category === filter);

  return (
    <div className="page">
      <PageHeader
        title="Homebrew"
        subtitle="Official releases of popular PS4 homebrew, straight from each project's GitHub page."
        actions={
          <Button icon="retry" busy={loading} onClick={() => load(true)}>
            Check for updates
          </Button>
        }
      />

      <div className="card homebrew-tip">
        <Icon name="package" size={18} className="homebrew-tip-icon" />
        <p className="small">
          <strong>Installing on the console:</strong> copy <span className="mono">.pkg</span> files to the root of an
          exFAT USB drive and open them from Debug Settings › Game › Package Installer (or GoldHEN's Package Installer).{" "}
          <span className="mono">.bin</span> files are payloads you send with GoldHEN's BinLoader or Payload Guest.
        </p>
      </div>

      <div className="segmented" role="radiogroup" aria-label="Category">
        {FILTERS.map((f) => (
          <button key={f.value} type="button" role="radio" aria-checked={filter === f.value} onClick={() => setFilter(f.value)}>
            {f.label}
          </button>
        ))}
      </div>

      {error && <ErrorNotice error={error} />}

      {apps.length === 0 ? (
        loading ? (
          <EmptyState icon="package" title="Loading homebrew…">
            Reading the newest releases from GitHub.
          </EmptyState>
        ) : (
          !error && <EmptyState icon="package" title="No homebrew found" />
        )
      ) : (
        <ul className="homebrew-list">
          {shown.map((app) => (
            <li key={app.id}>
              <HomebrewCard app={app} onNavigate={onNavigate} />
            </li>
          ))}
        </ul>
      )}

      <p className="muted small attribution">Files are downloaded directly from GitHub releases.</p>
    </div>
  );
}

function HomebrewCard({ app, onNavigate }: { app: HomebrewApp; onNavigate: (page: Page) => void }) {
  const [showNotes, setShowNotes] = useState(false);
  const release = app.release;
  const published = release?.publishedAt ? formatReleaseDate(release.publishedAt.slice(0, 10)) : "";

  return (
    <article className="card homebrew-card">
      <div className="homebrew-head">
        <div className="homebrew-title">
          <h2>{app.name}</h2>
          <span className="homebrew-category">{CATEGORY_LABELS[app.category]}</span>
        </div>
        <button type="button" className="link-button" onClick={() => runAction(() => api.openExternal(release?.url ?? app.repoUrl))}>
          View on GitHub
        </button>
      </div>
      <p className="muted small">{app.description}</p>

      {app.error ? (
        <ErrorNotice error={app.error} tone="warning" />
      ) : release ? (
        <>
          <p className="small homebrew-version">
            <Icon name="clock" size={14} /> Version {release.version}
            {published && ` · ${published}`}
          </p>
          <ul className="homebrew-assets">
            {release.assets.map((asset) => (
              <li key={asset.url}>
                <AssetRow asset={asset} onNavigate={onNavigate} />
              </li>
            ))}
          </ul>
          {release.notes && (
            <>
              <button type="button" className="link-button" onClick={() => setShowNotes((s) => !s)} aria-expanded={showNotes}>
                {showNotes ? "Hide release notes" : "Release notes"}
              </button>
              {showNotes && <pre className="homebrew-notes">{release.notes}</pre>}
            </>
          )}
        </>
      ) : (
        <p className="muted small">This project has no downloadable release right now.</p>
      )}
    </article>
  );
}

function AssetRow({ asset, onNavigate }: { asset: HomebrewAsset; onNavigate: (page: Page) => void }) {
  const added = useHomebrew((s) => Boolean(s.added[asset.url]));
  const markAdded = useHomebrew((s) => s.markAdded);
  const [busy, setBusy] = useState(false);
  const ext = asset.name.split(".").pop()?.toLowerCase() ?? "";

  const download = async () => {
    setBusy(true);
    const ok = await runAction(() => api.addDownload(asset.url, asset.checksum ?? undefined));
    setBusy(false);
    if (ok) markAdded(asset.url);
  };

  return (
    <div className="homebrew-asset">
      <Icon name={ext === "pkg" ? "package" : "file"} size={17} className="homebrew-asset-icon" />
      <span className="homebrew-asset-name">
        <span className="mono">{asset.name}</span>
        <span className="muted small">
          {formatBytes(asset.size)}
          {asset.checksum && " · checksum verified after download"}
        </span>
      </span>
      {added ? (
        <Button size="sm" variant="ghost" icon="check" onClick={() => onNavigate("downloads")}>
          Added · view
        </Button>
      ) : (
        <Button size="sm" variant="primary" icon="download" busy={busy} onClick={download}>
          Download
        </Button>
      )}
    </div>
  );
}
