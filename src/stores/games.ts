import { create } from "zustand";
import { api, toAppError } from "../lib/api";
import type { AppError, GameDetails, GameSummary, PlatformFilter } from "../types";

interface GamesState {
  query: string;
  platform: PlatformFilter;
  results: GameSummary[];
  count: number;
  nextPage: number | null;
  searching: boolean;
  loadingMore: boolean;
  error: AppError | null;
  /** Game shown on the details view; null shows the search results. */
  selectedId: number | null;
  details: Record<number, GameDetails>;
  detailsError: AppError | null;
  setQuery: (query: string) => void;
  setPlatform: (platform: PlatformFilter) => void;
  search: () => Promise<void>;
  loadMore: () => Promise<void>;
  open: (id: number) => Promise<void>;
  close: () => void;
}

// Ignores responses to searches that were superseded while in flight.
let searchSeq = 0;

export const useGames = create<GamesState>((set, get) => ({
  query: "",
  platform: "ps4",
  results: [],
  count: 0,
  nextPage: null,
  searching: false,
  loadingMore: false,
  error: null,
  selectedId: null,
  details: {},
  detailsError: null,

  setQuery: (query) => set({ query }),
  setPlatform: (platform) => set({ platform }),

  search: async () => {
    const { query, platform } = get();
    const seq = ++searchSeq;
    if (!query.trim()) {
      set({ results: [], count: 0, nextPage: null, searching: false, error: null });
      return;
    }
    set({ searching: true, error: null });
    try {
      const page = await api.searchGames(query, 1, platform);
      if (seq !== searchSeq) return;
      set({ results: page.results, count: page.count, nextPage: page.nextPage, searching: false });
    } catch (err) {
      if (seq !== searchSeq) return;
      set({ error: toAppError(err), searching: false });
    }
  },

  loadMore: async () => {
    const { query, platform, nextPage, loadingMore } = get();
    if (!nextPage || loadingMore) return;
    const seq = searchSeq;
    set({ loadingMore: true });
    try {
      const page = await api.searchGames(query, nextPage, platform);
      if (seq !== searchSeq) return;
      set((s) => {
        const seen = new Set(s.results.map((g) => g.id));
        return {
          results: [...s.results, ...page.results.filter((g) => !seen.has(g.id))],
          nextPage: page.nextPage,
          loadingMore: false,
        };
      });
    } catch (err) {
      if (seq !== searchSeq) return;
      set({ error: toAppError(err), loadingMore: false });
    }
  },

  open: async (id) => {
    set({ selectedId: id, detailsError: null });
    if (get().details[id]) return;
    try {
      const game = await api.getGame(id);
      set((s) => ({ details: { ...s.details, [id]: game } }));
    } catch (err) {
      if (get().selectedId === id) set({ detailsError: toAppError(err) });
    }
  },

  close: () => set({ selectedId: null, detailsError: null }),
}));
