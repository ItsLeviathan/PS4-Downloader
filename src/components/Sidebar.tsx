import logoMark from "../assets/logo-mark.png";
import { useStatusCounts, useTotalSpeed } from "../hooks/useDownloads";
import { APP_NAME } from "../lib/app";
import { formatSpeed } from "../lib/format";
import type { Page } from "../types";
import { Icon, type IconName } from "./Icon";

interface NavItem {
  page: Page;
  label: string;
  icon: IconName;
}

const GROUPS: { label: string; items: NavItem[] }[] = [
  {
    label: "Library",
    items: [
      { page: "dashboard", label: "Home", icon: "dashboard" },
      { page: "games", label: "Games", icon: "gamepad" },
    ],
  },
  {
    label: "Transfers",
    items: [
      { page: "downloads", label: "Downloads", icon: "download" },
      { page: "completed", label: "Completed", icon: "check" },
      { page: "history", label: "History", icon: "history" },
    ],
  },
  {
    label: "System",
    items: [
      { page: "storage", label: "Storage", icon: "drive" },
      { page: "settings", label: "Settings", icon: "settings" },
    ],
  },
];

export function Sidebar({ current, onNavigate }: { current: Page; onNavigate: (page: Page) => void }) {
  const counts = useStatusCounts();
  const speed = useTotalSpeed();
  const pending = counts.active + counts.paused + counts.failed;

  return (
    <nav className="sidebar" aria-label="Main">
      <button type="button" className="brand" onClick={() => onNavigate("dashboard")} aria-label={`${APP_NAME} home`}>
        <img className="brand-logo" src={logoMark} alt="" width={36} height={36} />
        <span className="brand-text">
          <span className="brand-name">PS4</span>
          <span className="brand-sub">Downloader</span>
        </span>
      </button>

      <div className="nav-groups">
        {GROUPS.map((group) => (
          <div key={group.label} className="nav-group">
            <p className="nav-label">{group.label}</p>
            <ul className="nav">
              {group.items.map((item) => (
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
          </div>
        ))}
      </div>

      <div className={`sidebar-speed${speed > 0 ? " is-active" : ""}`} aria-live="off">
        <span className="sidebar-speed-icon">
          <Icon name="download" size={16} strokeWidth={2.25} />
        </span>
        <span className="sidebar-speed-text">
          <span className="sidebar-speed-value">{speed > 0 ? formatSpeed(speed) : "Idle"}</span>
          <span className="sidebar-speed-label">{speed > 0 ? `${counts.active} active` : "No transfers running"}</span>
        </span>
      </div>
    </nav>
  );
}
