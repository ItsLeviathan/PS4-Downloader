import { useMemo } from "react";
import logoMark from "../assets/logo-mark.png";
import { AddDownloadForm } from "../components/AddDownloadForm";
import { Button } from "../components/Button";
import { DownloadCard } from "../components/DownloadCard";
import { Icon, type IconName } from "../components/Icon";
import { EmptyState, Section } from "../components/Layout";
import { StorageCard } from "../components/StorageCard";
import { ACTIVE_STATUSES, useDownloadsByStatus, useTotalSpeed } from "../hooks/useDownloads";
import { api } from "../lib/api";
import { formatBytes, formatDate, formatPercent, formatSpeed } from "../lib/format";
import { runAction, useApp } from "../stores/app";
import type { DownloadStatus, Page } from "../types";

const PAUSED: DownloadStatus[] = ["paused"];
const COMPLETED: DownloadStatus[] = ["completed"];
const DASHBOARD_LIMIT = 4;
const RECENT_LIMIT = 4;

function Hero({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const root = useApp((s) => s.settings?.storageRoot);
  return (
    <section className="hero" aria-label="Add a download">
      <div className="hero-body">
        <div className="hero-text">
          <h1>Ready when you are</h1>
          <p>
            Paste a direct download link and it goes straight to <span className="hero-path">{root}</span>.
          </p>
        </div>
        <AddDownloadForm bare />
        <button type="button" className="hero-link" onClick={() => onNavigate("games")}>
          <Icon name="search" size={15} /> Look up a game first
        </button>
      </div>
      <img className="hero-logo" src={logoMark} alt="" />
    </section>
  );
}

function IncompleteBanner() {
  const paused = useDownloadsByStatus(PAUSED);
  if (paused.length === 0) return null;
  return (
    <section className="card banner" aria-label="Incomplete downloads">
      <div className="banner-icon">
        <Icon name="pause" size={18} />
      </div>
      <div className="banner-main">
        <h2>
          {paused.length} paused {paused.length === 1 ? "download" : "downloads"}
        </h2>
        <ul className="banner-list">
          {paused.slice(0, 3).map((d) => {
            const pct = formatPercent(d.downloaded, d.totalSize);
            return (
              <li key={d.id}>
                <span className="banner-name">{d.fileName}</span>
                <span className="muted">{pct === null ? formatBytes(d.downloaded) : `${pct}%`}</span>
              </li>
            );
          })}
          {paused.length > 3 && <li className="muted">and {paused.length - 3} more</li>}
        </ul>
      </div>
      <Button variant="primary" icon="play" onClick={() => runAction(api.resumeAll)}>
        Resume all
      </Button>
    </section>
  );
}

function Stat({ icon, value, label, tone }: { icon: IconName; value: string | number; label: string; tone: string }) {
  return (
    <div className="stat" data-tone={tone}>
      <span className="stat-icon">
        <Icon name={icon} size={18} />
      </span>
      <span className="stat-text">
        <span className="stat-value">{value}</span>
        <span className="stat-label">{label}</span>
      </span>
    </div>
  );
}

function Stats() {
  const active = useDownloadsByStatus(ACTIVE_STATUSES);
  const completed = useDownloadsByStatus(COMPLETED);
  const speed = useTotalSpeed();
  const running = active.filter((d) => d.status !== "queued").length;
  return (
    <section className="stats" aria-label="Summary">
      <Stat icon="download" tone="blue" value={speed > 0 ? formatSpeed(speed) : "—"} label="Current speed" />
      <Stat icon="play" tone="cyan" value={running} label="Downloading" />
      <Stat icon="history" tone="amber" value={active.length - running} label="In queue" />
      <Stat icon="check" tone="green" value={completed.length} label="Completed" />
    </section>
  );
}

function RecentCard({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const completed = useDownloadsByStatus(COMPLETED);
  const recent = useMemo(
    () => [...completed].sort((a, b) => (b.completedAt ?? 0) - (a.completedAt ?? 0)).slice(0, RECENT_LIMIT),
    [completed],
  );
  return (
    <section className="card recent-card" aria-label="Recently completed">
      <div className="card-head">
        <div className="card-title">
          <Icon name="check" size={18} />
          <h2>Recently completed</h2>
        </div>
        {completed.length > 0 && (
          <button type="button" className="link-button" onClick={() => onNavigate("completed")}>
            See all
          </button>
        )}
      </div>
      {recent.length === 0 ? (
        <p className="muted small">Finished downloads will show up here.</p>
      ) : (
        <ul className="recent-list">
          {recent.map((d) => (
            <li key={d.id}>
              <span className="recent-dot" aria-hidden="true" />
              <span className="recent-main">
                <span className="recent-name" title={d.fileName}>
                  {d.fileName}
                </span>
                <span className="muted small">
                  {d.totalSize !== null && `${formatBytes(d.totalSize)} · `}
                  {formatDate(d.completedAt)}
                </span>
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const active = useDownloadsByStatus(ACTIVE_STATUSES);
  return (
    <div className="page">
      <Hero onNavigate={onNavigate} />
      <IncompleteBanner />
      <Stats />
      <div className="dash-grid">
        <Section
          title="Active downloads"
          count={active.length}
          actions={
            active.length > DASHBOARD_LIMIT && (
              <Button size="sm" variant="ghost" onClick={() => onNavigate("downloads")}>
                View all
              </Button>
            )
          }
        >
          {active.length === 0 ? (
            <EmptyState icon="download" title="No active downloads">
              Paste a direct download link above to get started.
            </EmptyState>
          ) : (
            <div className="stack">
              {active.slice(0, DASHBOARD_LIMIT).map((d) => (
                <DownloadCard key={d.id} record={d} />
              ))}
            </div>
          )}
        </Section>
        <aside className="dash-side">
          <StorageCard />
          <RecentCard onNavigate={onNavigate} />
        </aside>
      </div>
    </div>
  );
}
