// Mirrors the serialized Rust types in src-tauri/src.

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "retrying"
  | "verifying"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type Integrity = "notChecked" | "verified" | "failed";

export type ErrorCode =
  | "invalidUrl"
  | "unsupportedProtocol"
  | "network"
  | "timeout"
  | "notFound"
  | "forbidden"
  | "authRequired"
  | "rateLimited"
  | "serverError"
  | "httpStatus"
  | "notAFile"
  | "redirect"
  | "insufficientSpace"
  | "storageUnavailable"
  | "io"
  | "checksumMismatch"
  | "sizeMismatch"
  | "invalidChecksum"
  | "invalidSettings"
  | "duplicate"
  | "invalidState"
  | "unknownDownload"
  | "rangeNotHonored"
  | "apiKey"
  | "internal";

export interface AppError {
  code: ErrorCode;
  message: string;
  details?: string | null;
}

export interface ExpectedChecksum {
  algorithm: "sha256" | "sha1" | "md5";
  value: string;
}

export interface RetryInfo {
  attempt: number;
  maxAttempts: number;
  retryAt: number;
}

export interface DownloadRecord {
  id: string;
  url: string;
  fileName: string;
  partName: string;
  root: string;
  totalSize: number | null;
  downloaded: number;
  supportsRange: boolean;
  connections: number;
  validator: string | null;
  status: DownloadStatus;
  error: AppError | null;
  checksum: ExpectedChecksum | null;
  integrity: Integrity;
  createdAt: number;
  completedAt: number | null;
  finalPath: string | null;
  retry: RetryInfo | null;
}

export interface ProgressItem {
  id: string;
  downloaded: number;
  total: number | null;
  speed: number;
  eta: number | null;
}

export type Theme = "dark" | "light" | "system";

export interface Settings {
  storageRoot: string;
  maxSimultaneous: number;
  connections: number;
  autoStart: boolean;
  notifyCompleted: boolean;
  notifyFailed: boolean;
  notifyStorage: boolean;
  theme: Theme;
  rawgApiKey: string;
}

export interface StorageInfo {
  root: string;
  available: boolean;
  driveAvailable: boolean;
  free: number;
  used: number;
  total: number;
}

export interface FolderUsage {
  files: number;
  bytes: number;
}

export interface StorageUsage {
  completed: FolderUsage;
  partial: FolderUsage;
}

export type StorageFolder = "root" | "completed" | "incomplete";

export type Page = "dashboard" | "games" | "downloads" | "completed" | "history" | "storage" | "settings";

export type PlatformFilter = "ps4" | "ps5" | "all";

export interface GameSummary {
  id: number;
  slug: string;
  name: string;
  released: string | null;
  tba: boolean;
  image: string | null;
  rating: number;
  ratingsCount: number;
  metacritic: number | null;
  genres: string[];
  platforms: string[];
}

export interface GameSearchPage {
  results: GameSummary[];
  count: number;
  nextPage: number | null;
}

export interface PlatformRelease {
  name: string;
  releasedAt: string | null;
}

export interface Trailer {
  name: string;
  preview: string | null;
  url: string;
}

export interface GameDetails {
  id: number;
  slug: string;
  name: string;
  alternativeNames: string[];
  description: string;
  released: string | null;
  tba: boolean;
  image: string | null;
  imageAdditional: string | null;
  website: string | null;
  rating: number;
  ratingTop: number;
  ratingsCount: number;
  metacritic: number | null;
  playtime: number;
  esrb: string | null;
  genres: string[];
  platforms: PlatformRelease[];
  developers: string[];
  publishers: string[];
  stores: string[];
  tags: string[];
  screenshots: string[];
  trailers: Trailer[];
  rawgUrl: string;
}
