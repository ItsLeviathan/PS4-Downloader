import { useEffect } from "react";
import type { Theme } from "../types";

/** Applies the theme setting to <html data-theme>, following the OS for "system". */
export function useTheme(theme: Theme | undefined) {
  useEffect(() => {
    const root = document.documentElement;
    if (theme !== "system") {
      root.dataset.theme = theme ?? "dark";
      return;
    }
    const query = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => (root.dataset.theme = query.matches ? "light" : "dark");
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [theme]);
}
