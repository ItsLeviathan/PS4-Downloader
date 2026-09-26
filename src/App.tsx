import { useEffect, useState } from "react";
import logoMark from "./assets/logo-mark.png";
import { ErrorNotice } from "./components/ErrorNotice";
import { Toasts } from "./components/Layout";
import { Sidebar } from "./components/Sidebar";
import { StorageWarning } from "./components/StorageCard";
import { useTheme } from "./hooks/useTheme";
import { Completed } from "./pages/Completed";
import { Dashboard } from "./pages/Dashboard";
import { Downloads } from "./pages/Downloads";
import { Games } from "./pages/Games";
import { History } from "./pages/History";
import { Settings } from "./pages/Settings";
import { Storage } from "./pages/Storage";
import { useApp } from "./stores/app";
import type { Page } from "./types";

export function App() {
  const [page, setPage] = useState<Page>("dashboard");
  const ready = useApp((s) => s.ready);
  const loadError = useApp((s) => s.loadError);
  const storage = useApp((s) => s.storage);
  const theme = useApp((s) => s.settings?.theme);
  useTheme(theme);

  useEffect(() => {
    let cleanup: (() => void) | undefined;
    let disposed = false;
    useApp
      .getState()
      .init()
      .then((fn) => (disposed ? fn() : (cleanup = fn)));
    return () => {
      disposed = true;
      cleanup?.();
    };
  }, []);

  if (!ready) {
    return (
      <div className="boot" aria-busy="true">
        <img src={logoMark} alt="" />
      </div>
    );
  }

  return (
    <div className="app">
      <Sidebar current={page} onNavigate={setPage} />
      <main className="content">
        {loadError && <ErrorNotice error={loadError} />}
        {/* The dashboard and storage pages show this inside the storage card. */}
        {storage && page !== "dashboard" && page !== "storage" && <StorageWarning storage={storage} />}
        {page === "dashboard" && <Dashboard onNavigate={setPage} />}
        {page === "games" && <Games onNavigate={setPage} />}
        {page === "downloads" && <Downloads />}
        {page === "completed" && <Completed />}
        {page === "history" && <History />}
        {page === "storage" && <Storage />}
        {page === "settings" && <Settings />}
      </main>
      <Toasts />
    </div>
  );
}
