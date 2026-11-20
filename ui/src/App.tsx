import { useEffect } from "preact/hooks";
import { Rail } from "./components/Rail";
import { Toasts } from "./components/Toasts";
import { UpdateBanner } from "./components/UpdateBanner";
import { PreviewDrawer } from "./components/PreviewDrawer";
import { SearchView } from "./views/SearchView";
import { FavoritesView, CollectionsView } from "./views/LibraryViews";
import { HistoryView } from "./views/HistoryView";
import { DownloadsView } from "./views/DownloadsView";
import { SettingsView } from "./views/SettingsView";
import { DiagnosticsView } from "./views/DiagnosticsView";
import { StudioView } from "./views/StudioView";
import { onFocusSearch } from "./lib/api";
import { selected, view } from "./lib/store";

export function App() {
  useEffect(() => {
    const un = onFocusSearch(() => {
      view.value = "search";
      requestAnimationFrame(() => document.getElementById("q")?.focus());
    });
    const onKey = (e: KeyboardEvent) => {
      // Ctrl+K o "/" enfocan el buscador desde cualquier pantalla.
      const typing = (e.target as HTMLElement)?.closest("input, textarea, select");
      if ((e.ctrlKey && e.key.toLowerCase() === "k") || (!typing && e.key === "/")) {
        e.preventDefault();
        view.value = "search";
        requestAnimationFrame(() => document.getElementById("q")?.focus());
      } else if (e.key === "Escape" && selected.value && !typing) {
        selected.value = null;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const v = view.value;
  return (
    <div class="shell">
      <Rail />
      <main class="main">
        {v === "search" && <SearchView />}
        {v === "favorites" && <FavoritesView />}
        {v === "collections" && <CollectionsView />}
        {v === "history" && <HistoryView />}
        {v === "downloads" && <DownloadsView />}
        {v === "studio" && <StudioView />}
        {v === "settings" && <SettingsView />}
        {v === "diagnostics" && <DiagnosticsView />}
        {selected.value && <PreviewDrawer key={selected.value.uid} item={selected.value} />}
      </main>
      <UpdateBanner />
      <Toasts />
    </div>
  );
}
