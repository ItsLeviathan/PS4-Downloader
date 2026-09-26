import { invoke } from "@tauri-apps/api/core";
import type {
  AppError,
  DownloadRecord,
  GameDetails,
  GameSearchPage,
  PlatformFilter,
  Settings,
  StorageFolder,
  StorageInfo,
  StorageUsage,
} from "../types";

/** Normalizes anything thrown by `invoke` into an `AppError`. */
export function toAppError(err: unknown): AppError {
  if (err && typeof err === "object" && "message" in err && "code" in err) {
    return err as AppError;
  }
  return {
    code: "internal",
    message: "Something went wrong inside PS4 Downloader.",
    details: String(err),
  };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toAppError(err);
  }
}

export const api = {
  getSettings: () => call<Settings>("get_settings"),
  updateSettings: (settings: Settings) => call<Settings>("update_settings", { settings }),
  getStorageInfo: () => call<StorageInfo>("get_storage_info"),
  getStorageUsage: () => call<StorageUsage>("get_storage_usage"),
  createStorageFolder: () => call<StorageInfo>("create_storage_folder"),
  listDownloads: () => call<DownloadRecord[]>("list_downloads"),
  addDownload: (url: string, checksum?: string) =>
    call<DownloadRecord>("add_download", { url, checksum: checksum?.trim() || null }),
  pause: (id: string) => call<void>("pause_download", { id }),
  resume: (id: string) => call<void>("resume_download", { id }),
  retry: (id: string) => call<void>("retry_download", { id }),
  cancel: (id: string) => call<void>("cancel_download", { id }),
  remove: (id: string, deleteFile: boolean) => call<void>("remove_download", { id, deleteFile }),
  pauseAll: () => call<void>("pause_all"),
  resumeAll: () => call<void>("resume_all"),
  reveal: (id: string) => call<void>("reveal_download", { id }),
  openFolder: (folder: StorageFolder) => call<void>("open_storage_folder", { folder }),
  searchGames: (query: string, page: number, platform: PlatformFilter) =>
    call<GameSearchPage>("search_games", { query, page, platform }),
  getGame: (id: number) => call<GameDetails>("get_game", { id }),
  openExternal: (url: string) => call<void>("open_external", { url }),
};
