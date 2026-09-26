import { useStatusCounts, useTotalSpeed } from "../hooks/useDownloads";
import { APP_NAME } from "../lib/app";
import { formatSpeed } from "../lib/format";
import type { Page } from "../types";
import { Icon, type IconName } from "./Icon";

const NAV: { page: Page; label: string; icon: IconName }[] = [
  { page: "dashboard", label: "Dashboard", icon: "dashboard" },
  { page: "downloads", label: "Downloads", icon: "download" },
  { page: "completed", label: "Completed", icon: "check" },
  { page: "history", label: "History", icon: "history" },
  { page: "storage", label: "Storage", icon: "drive" },
  { page: "settings", label: "Settings", icon: "settings" },
];

export function Sidebar({ current, onNavigate }: { current: Page; onNavigate: (page: Page) => void }) {
  const counts = useStatusCounts();
  const speed = useTotalSpeed();
  const pending = counts.active + counts.paused + counts.failed;

  return (
    <nav className="sidebar" aria-label="Main">
      <div className="brand">
        <div className="brand-mark">
          <Icon name="download" size={18} strokeWidth={2.25} />
        </div>
        <span className="brand-name">{APP_NAME}</span>
      </div>

      <ul className="nav">
        {NAV.map((item) => (
          <li key={item.page}>
            <button
              type="button"
              className="nav-item"
              aria-current={current === item.page ? "page" : undefined}
              onClick={() => onNavigate(item.page)}
            >
              <Icon name={item.icon} size={18} />
              <span>{item.label}</span>
              {item.page === "downloads" && pending > 0 && <span className="nav-count">{pending}</span>}
            </button>
          </li>
        ))}
      </ul>

      {speed > 0 && (
        <div className="sidebar-speed" aria-live="off">
          <span className="muted small">Total speed</span>
          <span className="sidebar-speed-value">{formatSpeed(speed)}</span>
        </div>
      )}
    </nav>
  );
}
