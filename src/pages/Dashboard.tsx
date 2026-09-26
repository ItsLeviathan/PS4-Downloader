import { AddDownloadForm } from "../components/AddDownloadForm";
import { Button } from "../components/Button";
import { DownloadCard } from "../components/DownloadCard";
import { EmptyState, PageHeader, Section } from "../components/Layout";
import { StorageCard } from "../components/StorageCard";
import { ACTIVE_STATUSES, useDownloadsByStatus, useTotalSpeed } from "../hooks/useDownloads";
import { api } from "../lib/api";
import { APP_NAME } from "../lib/app";
import { formatBytes, formatPercent, formatSpeed } from "../lib/format";
import { runAction } from "../stores/app";
import type { DownloadStatus, Page } from "../types";

const PAUSED: DownloadStatus[] = ["paused"];
const COMPLETED: DownloadStatus[] = ["completed"];
const DASHBOARD_LIMIT = 4;

function IncompleteBanner() {
  const paused = useDownloadsByStatus(PAUSED);
  if (paused.length === 0) return null;
  return (
    <section className="card banner" aria-label="Incomplete downloads">
      <div className="banner-main">
        <h2>Incomplete downloads</h2>
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

function SummaryCard() {
  const active = useDownloadsByStatus(ACTIVE_STATUSES);
  const completed = useDownloadsByStatus(COMPLETED);
  const speed = useTotalSpeed();
  const running = active.filter((d) => d.status !== "queued").length;
  return (
    <section className="card summary-card" aria-label="Summary">
      <div className="summary-item">
        <span className="summary-value">{formatSpeed(speed)}</span>
        <span className="muted small">Current speed</span>
      </div>
      <div className="summary-item">
        <span className="summary-value">{running}</span>
        <span className="muted small">Downloading</span>
      </div>
      <div className="summary-item">
        <span className="summary-value">{active.length - running}</span>
        <span className="muted small">Waiting</span>
      </div>
      <div className="summary-item">
        <span className="summary-value">{completed.length}</span>
        <span className="muted small">Completed</span>
      </div>
    </section>
  );
}

export function Dashboard({ onNavigate }: { onNavigate: (page: Page) => void }) {
  const active = useDownloadsByStatus(ACTIVE_STATUSES);
  return (
    <div className="page">
      <PageHeader title={APP_NAME} subtitle="Download large files straight to your game drive." />
      <AddDownloadForm />
      <IncompleteBanner />
      <div className="dash-grid">
        <StorageCard />
        <SummaryCard />
      </div>
      <Section
        title="Downloads"
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
    </div>
  );
}
