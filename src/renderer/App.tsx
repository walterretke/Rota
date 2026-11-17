import { useCallback, useEffect, useState } from "react";
import type { AppStatus } from "../shared/ipc";
import { call, on } from "./ipc";
import { KeyBanner } from "./components/KeyBanner";
import { HomePage } from "./pages/HomePage";
import { SettingsPage } from "./pages/SettingsPage";

type Page = "home" | "settings";

export function App() {
  const [page, setPage] = useState<Page>("home");
  const [status, setStatus] = useState<AppStatus | null>(null);

  const refresh = useCallback(() => {
    call("get_app_status").then(setStatus).catch(() => undefined);
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = [
      on("lcu-state", (lcu) => setStatus((s) => (s ? { ...s, lcu } : s))),
      on("key-state", (keyState) => setStatus((s) => (s ? { ...s, keyState } : s))),
      on("import-progress", (p) => {
        setStatus((s) => (s ? { ...s, lastImport: p, importing: !p.finished } : s));
        if (p.finished || p.done % 10 === 0) refresh();
      }),
    ];
    return () => {
      unlisten.forEach((u) => u.then((f) => f()));
    };
  }, [refresh]);

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">Rota</div>
        <nav>
          <button className={page === "home" ? "tab active" : "tab"} onClick={() => setPage("home")}>
            Início
          </button>
          <button className={page === "settings" ? "tab active" : "tab"} onClick={() => setPage("settings")}>
            Configurações
          </button>
        </nav>
      </header>
      {status && <KeyBanner keyState={status.keyState} onOpenSettings={() => setPage("settings")} />}
      <main>
        {page === "home" ? (
          <HomePage status={status} onRefresh={refresh} />
        ) : (
          <SettingsPage onSaved={refresh} />
        )}
      </main>
    </div>
  );
}
