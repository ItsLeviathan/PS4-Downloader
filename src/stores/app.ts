import { listen } from "@tauri-apps/api/event";
import { create } from "zustand";
import { api, toAppError } from "../lib/api";
import type { AppError, DownloadRecord, ProgressItem, Settings, StorageInfo } from "../types";

export interface Toast {
  id: number;
  tone: "info" | "error";
  error: AppError;
}

interface AppState {
  ready: boolean;
  loadError: AppError | null;
  downloads: DownloadRecord[];
  /** Live transfer stats, only for running downloads. */
  progress: Record<string, ProgressItem>;
  settings: Settings | null;
  storage: StorageInfo | null;
  toasts: Toast[];
  init: () => Promise<() => void>;
  setSettings: (settings: Settings) => void;
  setStorage: (storage: StorageInfo) => void;
  showError: (err: unknown) => void;
  dismissToast: (id: number) => void;
}

const RUNNING = new Set(["downloading", "retrying", "verifying"]);
let toastId = 0;

export const useApp = create<AppState>((set, get) => ({
  ready: false,
  loadError: null,
  downloads: [],
  progress: {},
  settings: null,
  storage: null,
  toasts: [],

  init: async () => {
    // Subscribe first so no event between the initial fetch and subscription is lost.
    const unlisten = await Promise.all([
      listen<DownloadRecord>("download-updated", ({ payload }) => {
        set((s) => {
          const index = s.downloads.findIndex((d) => d.id === payload.id);
          const downloads =
            index === -1
              ? [...s.downloads, payload]
              : s.downloads.map((d, i) => (i === index ? payload : d));
          let progress = s.progress;
          if (!RUNNING.has(payload.status) && payload.id in progress) {
            progress = { ...progress };
            delete progress[payload.id];
          }
          return { downloads, progress };
        });
      }),
      listen<string>("download-removed", ({ payload }) => {
        set((s) => {
          const progress = { ...s.progress };
          delete progress[payload];
          return { downloads: s.downloads.filter((d) => d.id !== payload), progress };
        });
      }),
      listen<ProgressItem[]>("download-progress", ({ payload }) => {
        set((s) => {
          const progress = { ...s.progress };
          for (const item of payload) progress[item.id] = item;
          return { progress };
        });
      }),
      listen<StorageInfo>("storage-changed", ({ payload }) => set({ storage: payload })),
    ]);

    try {
      const [downloads, settings, storage] = await Promise.all([
        api.listDownloads(),
        api.getSettings(),
        api.getStorageInfo(),
      ]);
      set({ downloads, settings, storage, ready: true });
    } catch (err) {
      set({ loadError: toAppError(err), ready: true });
    }
    return () => unlisten.forEach((fn) => fn());
  },

  setSettings: (settings) => set({ settings }),
  setStorage: (storage) => set({ storage }),

  showError: (err) => {
    const toast: Toast = { id: ++toastId, tone: "error", error: toAppError(err) };
    set((s) => ({ toasts: [...s.toasts.slice(-3), toast] }));
    setTimeout(() => get().dismissToast(toast.id), 8000);
  },

  dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Runs a backend action and reports failures as a toast. */
export async function runAction(action: () => Promise<unknown>): Promise<boolean> {
  try {
    await action();
    return true;
  } catch (err) {
    useApp.getState().showError(err);
    return false;
  }
}
