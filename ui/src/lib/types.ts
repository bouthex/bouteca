// Tipos espejo de bouteca-core (serde, snake_case).

export type MediaKind = "gif" | "video" | "image" | "audio" | "music";
export type KindFilter = "all" | "gif" | "video" | "image" | "audio" | "music" | "meme";
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
  rights: Rights;
  variants: Variant[];
  notices?: string[];
  sensitive?: boolean;
  published_at?: number | null;
  score: number;
  dedupe_key: string;
}

export type LicenseKind =
  | "cc0" | "public_domain" | "cc_by" | "cc_by_sa" | "cc_by_nd" | "cc_by_nc" | "cc_by_nc_sa" | "cc_by_nc_nd"
  | "provider_free" | "platform_terms" | "restricted" | "unknown";

/** Permisos derivados por el motor (`bouteca-core::rights`). */
export interface Rights {
  license: LicenseKind;
  label: string;
  play: boolean;
  download: boolean;
  attribution_required: boolean;
  commercial: boolean | null;
  derivatives: boolean | null;
  share_alike: boolean;
  summary: string;
  indicator: Indicator;
  attribution: string;
}

export interface Variant {
  uid: string;
  provider: string;
  kind: MediaKind;
  title: string;
  reason: string;
  confidence: number;
  width: number | null;
  height: number | null;
  duration_secs: number | null;
  download: boolean;
  thumbnail: string | null;
}

export type RightsFilter = "sin_atribucion" | "con_atribucion" | "libre" | "licencia_comercial" | "restringido";
export type Usage = "personal" | "you_tube" | "you_tube_monetized" | "social" | "stream" | "game" | "film" | "commercial";
export type Indicator = "sin_atribucion" | "requiere_atribucion" | "licencia_comercial" | "uso_restringido" | "desconocida";

export interface Intent {
  cleaned: string;
  kind: KindFilter | null;
  rights: RightsFilter | null;
  usage: Usage | null;
  duration_secs: number | null;
  detected: string[];
}

export interface ItemDetails {
  rights: Rights;
  usages: { usage: Usage; label: string; ok: boolean | null }[];
  tiers: Tier[];
}

export const isAudioKind = (k: MediaKind) => k === "audio" || k === "music";

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
  | {
      type: "plan"; search_id: number; variants: QueryVariant[]; concepts: string[]; entities: string[]; providers: string[];
      intent: Intent; filter: KindFilter; rights: RightsFilter | null; usage: Usage | null;
    }
  | { type: "batch"; search_id: number; provider: string; items: MediaItem[] }
  | { type: "alternate"; search_id: number; dedupe_key: string; provider: string; uid: string; renditions: Rendition[] }
  | { type: "grouped"; search_id: number; group: string; uid: string; from_group: string | null; item: MediaItem | null; variant: Variant; duplicate: boolean; promote: boolean }
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
  /** Funciona sin clave propia gracias al servicio central de Bouteca. */
  via_relay: boolean;
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
  status: "queued" | "downloading" | "verifying" | "completed" | "failed" | "cancelled" | "interrupted" | "paused" | "missing";
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
  | { type: "updated"; row: DownloadRow }
  | { type: "removed"; id: number };

export interface DeleteOutcome { id: number; ok: boolean; file_deleted: boolean; error: string | null }
export interface ReconcileReport { checked: number; missing: number; relocated: number; restored: number }

export type EnqueueOutcome = { outcome: "queued"; id: number } | { outcome: "already_downloaded"; row: DownloadRow };

export interface EngineSettings {
  primary_lang: Lang;
  extra_langs: Lang[];
  disabled_providers: string[];
  safe_search: boolean;
  include_unlicensed: boolean;
  per_page: number;
  group_duplicates: boolean;
  visual_dedup: boolean;
  favorite_providers: string[];
}

export interface SavedFilter {
  name: string;
  data: { rights?: RightsFilter | null; usage?: Usage | null; filters?: import("./filters").ResultFilters };
}

export type QualityPref = "auto" | "original" | "alta" | "media" | "baja";

export interface Tier {
  tier: "Original" | "Alta" | "Media" | "Baja" | "Única";
  url: string;
  label: string;
  mime: string | null;
  width: number | null;
  height: number | null;
  size_bytes: number | null;
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
  preferred_quality: QualityPref;
  saved_filters: SavedFilter[];
  updates: { check_on_start: boolean; auto_download: boolean; channel: "stable" | "beta" };
  use_relay: boolean;
}

export type UpdateEvent =
  | { type: "checking" }
  | { type: "up_to_date"; current: string }
  | { type: "available"; version: string; notes: string; date: string | null }
  | { type: "progress"; done: number; total: number | null }
  | { type: "ready"; version: string }
  | { type: "error"; message: string };

export interface UpdateInfo { configured: boolean; current: string; available: string | null; notes: string; date: string | null; ready: string | null }
export interface ReleaseEntry { version: string; name: string; date: string | null; prerelease: boolean; notes: string; url: string }

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

export interface StudioProject { id: number; name: string; description: string; created_at: number; updated_at: number; count: number }
export interface KitCategory { id: "music" | "sfx" | "video" | "image"; label: string; filter: KindFilter; queries: string[] }
export interface KitPlan { theme: string; name: string; categories: KitCategory[]; rights: RightsFilter | null; usage: Usage | null; duration_secs: number | null }
export interface StudioDetail { project: StudioProject; plan: KitPlan; items: { category: string; item: MediaItem }[] }
