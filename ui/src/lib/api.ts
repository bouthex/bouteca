// Puente tipado hacia los comandos de Rust.
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type * as T from "./types";

const call = <R>(cmd: string, args?: Record<string, unknown>) => invoke<R>(cmd, args);

export const api = {
  appReady: () => call<void>("app_ready"),
  appInfo: () => call<T.AppInfo>("app_info"),
  providers: () => call<T.ProviderView[]>("providers_list"),
  relayRefresh: () => call<string[]>("relay_refresh"),
  clipboardImage: (url: string) => call<ArrayBuffer>("clipboard_image", { url }),
  setProviderKey: (provider: string, key: string | null) => call<void>("set_provider_key", { provider, key }),
  setProviderEnabled: (provider: string, enabled: boolean) => call<void>("set_provider_enabled", { provider, enabled }),

  search: (search_id: number, query: string, filter: T.KindFilter, page = 1, rights: T.RightsFilter | null = null, usage: T.Usage | null = null, max_duration: number | null = null) =>
    call<void>("search", { req: { search_id, query, filter, page, rights, usage, max_duration } }),
  studioList: () => call<T.StudioProject[]>("studio_list"),
  studioCreate: (description: string) => call<number>("studio_create", { description }),
  studioDelete: (id: number) => call<void>("studio_delete", { id }),
  studioDetail: (id: number) => call<T.StudioDetail>("studio_detail", { id }),
  studioSuggest: (id: number, category: string) => call<T.MediaItem[]>("studio_suggest", { id, category }),
  studioAdd: (id: number, category: string, item: T.MediaItem) => call<void>("studio_add", { id, category, item }),
  studioRemove: (id: number, uid: string) => call<void>("studio_remove", { id, uid }),
  studioCredits: (id: number) => call<string>("studio_credits", { id }),
  updateCheck: (download: boolean) => call<T.UpdateInfo>("update_check", { download }),
  updateInstall: () => call<void>("update_install"),
  updateHistory: () => call<T.ReleaseEntry[]>("update_history"),
  importOwnFile: (item: T.MediaItem) => call<T.DownloadRow | null>("import_own_file", { item }),
  downloadableAlternatives: (item: T.MediaItem) => call<T.MediaItem[]>("downloadable_alternatives", { item }),
  itemDetails: (item: T.MediaItem) => call<T.ItemDetails>("item_details", { item }),
  cancelSearch: (search_id: number) => call<void>("cancel_search", { searchId: search_id }),
  resolve: (item: T.MediaItem) => call<T.MediaItem>("resolve_item", { item }),
  suggestions: (prefix: string) => call<string[]>("suggestions", { prefix }),
  recordView: (item: T.MediaItem) => call<void>("record_view", { item }),

  favorites: () => call<T.SavedItem[]>("favorites_list"),
  favoriteUids: () => call<string[]>("favorite_uids"),
  favoriteAdd: (item: T.MediaItem) => call<void>("favorite_add", { item }),
  favoriteRemove: (uid: string) => call<boolean>("favorite_remove", { uid }),

  collections: () => call<T.Collection[]>("collections_list"),
  collectionCreate: (name: string) => call<number>("collection_create", { name }),
  collectionRename: (id: number, name: string) => call<void>("collection_rename", { id, name }),
  collectionDelete: (id: number) => call<void>("collection_delete", { id }),
  collectionAdd: (id: number, item: T.MediaItem) => call<void>("collection_add", { id, item }),
  collectionRemove: (id: number, uid: string) => call<void>("collection_remove", { id, uid }),
  collectionItems: (id: number) => call<T.SavedItem[]>("collection_items", { id }),
  collectionsOf: (uid: string) => call<number[]>("collections_of", { uid }),

  historySearches: () => call<T.SearchEntry[]>("history_searches"),
  historyViews: () => call<T.SavedItem[]>("history_views"),
  historyClear: (what: "searches" | "views" | "downloads" | "all") => call<void>("history_clear", { what }),

  download: (item: T.MediaItem, renditionUrl?: string) =>
    call<T.EnqueueOutcome>("download_start", { item, renditionUrl: renditionUrl ?? null }),
  downloads: () => call<T.DownloadRow[]>("downloads_list"),
  downloadCancel: (id: number) => call<void>("download_cancel", { id }),
  downloadsDelete: (ids: number[], delete_file: boolean) => call<T.DeleteOutcome[]>("downloads_delete", { ids, delete_file }),
  downloadsReconcile: () => call<T.ReconcileReport>("downloads_reconcile"),
  downloadPath: (id: number) => call<string>("download_path", { id }),
  downloadPause: (id: number) => call<void>("download_pause", { id }),
  downloadResume: (id: number) => call<void>("download_resume", { id }),
  downloadRetry: (id: number) => call<void>("download_retry", { id }),
  downloadRemove: (id: number) => call<void>("download_remove", { id }),
  downloadOpen: (id: number) => call<void>("download_open", { id }),
  downloadReveal: (id: number) => call<void>("download_reveal", { id }),
  openDownloadsFolder: () => call<void>("open_downloads_folder"),
  openExternal: (url: string) => call<void>("open_external", { url }),

  settings: () => call<T.AppSettings>("settings_get"),
  updateSettings: (settings: T.AppSettings) => call<T.AppSettings>("settings_update", { settings }),
  chooseDownloadDir: () => call<string | null>("choose_download_dir"),

  visualCompare: (image: number[], candidates: [string, string][]) => call<T.VisualReport>("visual_compare", { image, candidates }),
  diagnostics: () => call<T.Diagnostics>("diagnostics"),
  runCleanup: () => call<T.CleanupReport>("run_cleanup"),
  clearWebCache: () => call<void>("clear_web_cache"),
  setPlayersActive: (count: number) => call<void>("set_players_active", { count }),
  activity: () => call<void>("activity"),
};

export function onSearchEvent(cb: (e: T.SearchEvent) => void): Promise<UnlistenFn> {
  return listen<T.SearchEvent>("search-event", (e) => cb(e.payload));
}
export function onUpdateEvent(cb: (e: T.UpdateEvent) => void): Promise<UnlistenFn> {
  return listen<T.UpdateEvent>("update-event", (e) => cb(e.payload));
}
export function onDownloadEvent(cb: (e: T.DownloadEvent) => void): Promise<UnlistenFn> {
  return listen<T.DownloadEvent>("download-event", (e) => cb(e.payload));
}
export function onFocusSearch(cb: () => void): Promise<UnlistenFn> {
  return listen("focus-search", () => cb());
}

/** Mensaje de error legible desde lo que devuelva invoke. */
export function errText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Ocurrió un error inesperado";
}
