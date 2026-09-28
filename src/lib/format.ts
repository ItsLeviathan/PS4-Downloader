const UNITS = ["B", "KB", "MB", "GB", "TB", "PB"];

/** Binary units, labelled the way Windows Explorer labels them. */
export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit++;
  }
  return unit === 0 ? `${value} B` : `${value.toFixed(value >= 100 ? 0 : digits)} ${UNITS[unit]}`;
}

export function formatSpeed(bytesPerSecond: number): string {
  return `${formatBytes(bytesPerSecond)}/s`;
}

export function formatEta(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const s = Math.max(0, Math.round(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  const pad = (n: number) => n.toString().padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(sec)}` : `${pad(m)}:${pad(sec)}`;
}

export function formatPercent(done: number, total: number | null): number | null {
  if (!total) return null;
  return Math.min(100, Math.floor((done / total) * 100));
}

const dateFormat = new Intl.DateTimeFormat(undefined, {
  year: "numeric",
  month: "short",
  day: "numeric",
  hour: "2-digit",
  minute: "2-digit",
});

export function formatDate(ms: number | null | undefined): string {
  return ms ? dateFormat.format(new Date(ms)) : "—";
}

/** An API date ("2015-03-24", "2015-03" or "2015") as "Mar 24, 2015", "Mar 2015" or "2015". */
export function formatReleaseDate(isoDate: string | null | undefined): string {
  if (!isoDate) return "";
  const [y, m, d] = isoDate.split("-").map(Number);
  if (!y) return isoDate;
  if (!m) return String(y);
  const date = new Date(y, m - 1, d || 1);
  return d
    ? date.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" })
    : date.toLocaleDateString(undefined, { year: "numeric", month: "short" });
}
