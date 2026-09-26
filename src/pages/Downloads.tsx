import { Button } from "../components/Button";
import { DownloadCard } from "../components/DownloadCard";
import { EmptyState, PageHeader, Section } from "../components/Layout";
import { useDownloadsByStatus } from "../hooks/useDownloads";
import { api } from "../lib/api";
import { runAction, useApp } from "../stores/app";
import type { DownloadStatus } from "../types";

const RUNNING: DownloadStatus[] = ["downloading", "retrying", "verifying"];
const QUEUED: DownloadStatus[] = ["queued"];
const PAUSED: DownloadStatus[] = ["paused"];
const FAILED: DownloadStatus[] = ["failed"];

export function Downloads() {
  const running = useDownloadsByStatus(RUNNING);
  const queued = useDownloadsByStatus(QUEUED);
  const paused = useDownloadsByStatus(PAUSED);
  const failed = useDownloadsByStatus(FAILED);
  const maxSimultaneous = useApp((s) => s.settings?.maxSimultaneous);
  const empty = running.length + queued.length + paused.length + failed.length === 0;

  return (
    <div className="page">
      <PageHeader
        title="Downloads"
        subtitle={maxSimultaneous ? `Up to ${maxSimultaneous} at a time. The rest wait in the queue.` : undefined}
        actions={
          <>
            <Button icon="pause" disabled={running.length + queued.length === 0} onClick={() => runAction(api.pauseAll)}>
              Pause all
            </Button>
            <Button icon="play" disabled={paused.length === 0} onClick={() => runAction(api.resumeAll)}>
              Resume all
            </Button>
          </>
        }
      />

      {empty && (
        <EmptyState icon="download" title="Nothing downloading">
          Add a link from the Dashboard. Active, queued and paused downloads appear here.
        </EmptyState>
      )}
      {running.length > 0 && (
        <Section title="Active" count={running.length}>
          <div className="stack">
            {running.map((d) => (
              <DownloadCard key={d.id} record={d} />
            ))}
          </div>
        </Section>
      )}
      {queued.length > 0 && (
        <Section title="Queue" count={queued.length}>
          <div className="stack">
            {queued.map((d, i) => (
              <DownloadCard key={d.id} record={d} position={i + 1} />
            ))}
          </div>
        </Section>
      )}
      {paused.length > 0 && (
        <Section title="Paused" count={paused.length}>
          <div className="stack">
            {paused.map((d) => (
              <DownloadCard key={d.id} record={d} />
            ))}
          </div>
        </Section>
      )}
      {failed.length > 0 && (
        <Section title="Failed" count={failed.length}>
          <div className="stack">
            {failed.map((d) => (
              <DownloadCard key={d.id} record={d} />
            ))}
          </div>
        </Section>
      )}
    </div>
  );
}
