import { create } from "zustand";
import { api, toAppError } from "../lib/api";
import type { AppError, HomebrewApp } from "../types";

interface HomebrewState {
  apps: HomebrewApp[];
  loading: boolean;
  error: AppError | null;
  /** Asset URLs sent to the download manager this session. */
  added: Record<string, true>;
  load: (refresh?: boolean) => Promise<void>;
  markAdded: (url: string) => void;
}

export const useHomebrew = create<HomebrewState>((set, get) => ({
  apps: [],
  loading: false,
  error: null,
  added: {},

  load: async (refresh = false) => {
    if (get().loading) return;
    set({ loading: true, error: null });
    try {
      const apps = await api.listHomebrew(refresh);
      set({ apps, loading: false });
    } catch (err) {
      set({ error: toAppError(err), loading: false });
    }
  },

  markAdded: (url) => set((s) => ({ added: { ...s.added, [url]: true } })),
}));
