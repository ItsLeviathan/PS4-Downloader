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

export type Page = "dashboard" | "games" | "homebrew" | "downloads" | "completed" | "history" | "storage" | "settings";

export type PlatformFilter = "ps4" | "ps5" | "all";

export interface CriticScore {
  /** Out of 100. */
  score: number;
  source: string;
}

export interface GameSummary {
  /** Wikidata item id, e.g. "Q17154554". */
  id: string;
  name: string;
  summary: string | null;
  /** "YYYY-MM-DD", "YYYY-MM" or "YYYY". */
  released: string | null;
  image: string | null;
  criticScore: CriticScore | null;
  genres: string[];
  consoles: string[];
}

export interface GameSearchPage {
  results: GameSummary[];
  nextOffset: number | null;
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

export type Block = { type: "heading"; text: string } | { type: "text"; text: string };

export interface Section {
  title: string;
  blocks: Block[];
}

export interface RawgRating {
  rating: number;
  ratingTop: number;
  ratingsCount: number;
  url: string;
}

export interface GameDetails {
  id: string;
  name: string;
  summary: string | null;
  sections: Section[];
  released: string | null;
  image: string | null;
  backdrop: string | null;
  website: string | null;
  wikipediaUrl: string | null;
  wikidataUrl: string;
  criticScore: CriticScore | null;
  genres: string[];
  platforms: PlatformRelease[];
  developers: string[];
  publishers: string[];
  series: string[];
  modes: string[];
  ageRatings: string[];
  alternativeNames: string[];
  consoles: string[];
  screenshots: string[];
  trailers: Trailer[];
  rawg: RawgRating | null;
}

export type HomebrewCategory = "essentials" | "tools" | "payloads";

export interface HomebrewAsset {
  name: string;
  size: number;
  url: string;
  /** "sha256:<hex>" when GitHub publishes a digest for the file. */
  checksum: string | null;
}

export interface HomebrewRelease {
  version: string;
  publishedAt: string | null;
  notes: string | null;
  url: string;
  assets: HomebrewAsset[];
}

export interface HomebrewApp {
  id: string;
  name: string;
  category: HomebrewCategory;
  description: string;
  repoUrl: string;
  release: HomebrewRelease | null;
  error: AppError | null;
}
