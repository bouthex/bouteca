// Tipos espejo de bouteca-core (serde, snake_case).

export type MediaKind = "gif" | "video" | "image" | "audio";
export type KindFilter = "all" | "gif" | "video" | "image" | "audio" | "meme";
export type DownloadPolicy = "free" | "personal" | "preview_only";
export type Lang = "es" | "en" | "pt" | "tr" | "ja" | "ru";

export interface Rendition {
  label: string;
  url: string;
  mime: string | null;
  width: number | null;
  height: number | null;
  size_bytes: number | null;
  original: boolean;
}

export interface MediaItem {
  uid: string;
  provider: string;
  provider_id: string;
  kind: MediaKind;
  title: string;
  tags: string[];
  thumbnail: string | null;
  preview: string | null;
  page_url: string | null;
  author: string | null;
  license: string | null;
  license_url: string | null;
  width: number | null;
  height: number | null;
  duration_secs: number | null;
  popularity: number | null;
  renditions: Rendition[];
  needs_resolve: boolean;
  download_policy: DownloadPolicy;
  embed_url?: string | null;
  score: number;
  dedupe_key: string;
}

export interface QueryVariant {
  text: string;
  lang: Lang;
  weight: number;
  reason: string;
}

export type ProviderStatus =
  | { state: "ready" }
  | { state: "needs_key" }
  | { state: "disabled" }
  | { state: "rate_limited"; detail: string }
  | { state: "error"; detail: string };

export type SearchEvent =
  | { type: "plan"; search_id: number; variants: QueryVariant[]; concepts: string[]; entities: string[]; providers: string[] }
  | { type: "batch"; search_id: number; provider: string; items: MediaItem[] }
  | { type: "alternate"; search_id: number; dedupe_key: string; provider: string; uid: string; renditions: Rendition[] }
  | { type: "provider_error"; search_id: number; provider: string; message: string; status: ProviderStatus }
  | { type: "done"; search_id: number; has_more: boolean; total_items: number; elapsed_ms: number };

export interface ProviderInfo {
  id: string;
  name: string;
  kinds: MediaKind[];
  needs_key: boolean;
  key_url: string | null;
  homepage: string;
  rate_limit: string;
  terms: string;
  attribution_mark: string | null;
}

export interface ProviderView {
  info: ProviderInfo;
  status: ProviderStatus;
  has_key: boolean;
  enabled: boolean;
}

export interface SavedItem {
  item: MediaItem;
  added_at: number;
  available: boolean;
}

export interface Collection {
  id: number;
  name: string;
  count: number;
  cover: string | null;
}

export interface SearchEntry {
  query: string;
  filter: string;
  at: number;
}

export interface DownloadRow {
  id: number;
  uid: string;
  provider: string;
  title: string;
  kind: MediaKind;
  url: string;
  path: string | null;
  status: "queued" | "downloading" | "verifying" | "completed" | "failed" | "cancelled" | "interrupted";
  bytes_total: number | null;
  bytes_done: number;
  hash: string | null;
  mime: string | null;
  error: string | null;
  attribution: string | null;
  created_at: number;
  completed_at: number | null;
}

export type DownloadEvent =
  | { type: "progress"; id: number; done: number; total: number | null; speed_bps: number }
  | { type: "updated"; row: DownloadRow };

export type EnqueueOutcome = { outcome: "queued"; id: number } | { outcome: "already_downloaded"; row: DownloadRow };

export interface EngineSettings {
  primary_lang: Lang;
  extra_langs: Lang[];
  disabled_providers: string[];
  safe_search: boolean;
  include_unlicensed: boolean;
  per_page: number;
}

export interface AppSettings {
  engine: EngineSettings;
  download_dir: string | null;
  organize_by_kind: boolean;
  detect_duplicates: boolean;
  global_shortcut: string | null;
  minimize_to_tray: boolean;
  launch_at_startup: boolean;
  hover_previews: boolean;
  autoplay_gifs: boolean;
  web_cache_limit_mb: number;
  auto_cleanup: boolean;
  thumbnail_quality: string;
  theme: "dark" | "light";
  motion: "system" | "reduced" | "off";
}

export interface StorageReport {
  database_bytes: number;
  logs_bytes: number;
  web_cache_bytes: number;
  incomplete_bytes: number;
  incomplete_files: number;
  total_bytes: number;
}

export interface Diagnostics {
  ram_bytes: number;
  virtual_bytes: number;
  cpu_percent: number;
  webview_ram_bytes: number;
  storage: StorageReport;
  active_searches: number;
  active_downloads: number;
  active_players: number;
  cached_pages: number;
  uptime_secs: number;
}

export interface CleanupReport {
  removed_partials: number;
  removed_logs: number;
  freed_bytes: number;
  vacuumed: boolean;
  web_cache_over_limit: boolean;
}

export interface AppInfo {
  version: string;
  data_dir: string;
  downloads_dir: string;
  platform: string;
  shortcut_error: string | null;
}

export interface VisualReport {
  matches: { uid: string; similarity: number; near_duplicate: boolean }[];
  color: string | null;
  compared: number;
  failed: number;
}
