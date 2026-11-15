// Mirrors the Rust structs in src-tauri/src/library.rs and import.rs.

export type GameSource = "uploaded" | "archive";
export type Compat = "works" | "partial" | "projector" | "unknown";

export interface Game {
  id: number;
  title: string;
  developer: string | null;
  publisher: string | null;
  year: number | null;
  description: string | null;
  source: GameSource;
  /** Absolute path of the library's own copy. */
  swfPath: string;
  originalName: string;
  sha256: string;
  fileSize: number;
  swfVersion: number;
  compression: "none" | "zlib" | "lzma";
  width: number;
  height: number;
  frameRate: number;
  frameCount: number;
  asVersion: 1 | 2 | 3;
  usesNetwork: boolean;
  background: string | null;
  compat: Compat;
  /** Tile color for generated art until real art is scraped. */
  color: string;
  favorite: boolean;
  playSeconds: number;
  lastPlayed: string | null;
  addedAt: string;
  runtime: "ruffle" | "projector";
  allowNetwork: boolean;
  tags: string[];
  /** Absolute path of the image chosen for generated art (from inside the SWF or a screenshot). */
  fallbackImage: string | null;
  scaleMode: ScaleMode;
  quality: Quality;
  /** null = the SWF's own frame rate. */
  frameRateOverride: number | null;
  /** #rrggbb behind the game, null = black. */
  letterbox: string | null;
  series: string | null;
  releaseDate: string | null;
  scrapedAt: string | null;
  titleLocked: boolean;
  /** Selected image per slot (absolute paths). */
  art: ArtSet;
  /** Sources this game is matched to. */
  links: string[];
  /** Flashpoint id when the game was downloaded with its own extra files. */
  contentId: string | null;
  /** Where the main SWF originally lived; its loads resolve from there. */
  launchUrl: string | null;
  /** JSON object of controller button -> key code; null = default layout. */
  gamepadMap: string | null;
}

export type ArtSlot = "cover" | "wide" | "hero" | "logo" | "icon" | "screenshot" | "embedded";

export type ArtSet = Partial<Record<ArtSlot, string | null>>;

export type ScaleMode = "fit" | "stretch" | "original";
export type Quality = "low" | "medium" | "high" | "best";

export interface Collection {
  id: number;
  name: string;
  gameIds: number[];
}

export type GamePatch = Partial<
  Pick<
    Game,
    "title" | "favorite" | "developer" | "publisher" | "year" | "description" | "compat" | "scaleMode" | "quality" | "runtime" | "allowNetwork"
  >
> & {
  /** 0 clears the override. */
  frameRateOverride?: number;
  /** "" clears it. */
  letterbox?: string;
  /** JSON object; "" goes back to the default layout. */
  gamepadMap?: string;
};

export interface ArtItem {
  id: number;
  slot: ArtSlot;
  /** swf, player, user, flashpoint, steamgriddb, igdb, archive */
  source: string;
  /** Empty until a remote candidate is downloaded. */
  path: string;
  remoteUrl: string | null;
  thumbUrl: string | null;
  width: number | null;
  height: number | null;
  selected: boolean;
}

export interface ProjectorExit {
  gameId: number;
  seconds: number;
  error: string | null;
}

export interface SwfInfo {
  compression: "none" | "zlib" | "lzma";
  version: number;
  uncompressedLen: number;
  width: number;
  height: number;
  frameRate: number;
  frameCount: number;
  asVersion: 1 | 2 | 3;
  usesNetwork: boolean;
  background: string | null;
  metaTitle: string | null;
}

export interface ImportCandidate {
  path: string;
  fileName: string;
  fileSize: number;
  sha256: string | null;
  title: string;
  info: SwfInfo | null;
  error: string | null;
  duplicateOf: { id: number; title: string } | null;
  duplicateInBatch: boolean;
  preview: string | null;
  imageCount: number;
}

export interface ImportItem {
  path: string;
  title: string;
}

export interface ImportSummary {
  imported: Game[];
  failed: { path: string; error: string }[];
}

export interface ImportProgress {
  phase: "inspect" | "import";
  done: number;
  total: number;
  current: string;
}

export interface ScrapeMatch {
  source: string;
  id: string;
  title: string;
  altTitles: string[];
  developer: string | null;
  year: number | null;
  platform: string | null;
  thumb: string | null;
  score: number;
  fileMatch: boolean;
  payload?: unknown;
}

export interface SearchGroup {
  source: string;
  label: string;
  unavailable: string | null;
  error: string | null;
  matches: ScrapeMatch[];
  linkedId: string | null;
}

export interface ScrapeReport {
  gameId: number;
  title: string;
  linked: string[];
  needsReview: boolean;
  errors: string[];
}

export interface BulkReport {
  reports: ScrapeReport[];
  cancelled: boolean;
}

export interface ScrapeProgress {
  done: number;
  total: number;
  current: string;
  finished: boolean;
}

// ---------------------------------------------------------------- flashpoint

export type BrowseSort = "title" | "new" | "newest" | "oldest";
export type BrowseKind = "games" | "animations" | "all";

export interface BrowseGame {
  id: string;
  title: string;
  developer: string | null;
  publisher: string | null;
  series: string | null;
  releaseDate: string | null;
  year: number | null;
  description: string | null;
  tags: string[];
  playMode: string | null;
  language: string | null;
  logo: string | null;
  logoThumb: string | null;
  screenshot: string | null;
  /** Zip size; null for entries that are fetched file by file. */
  size: number | null;
  /** "page" games start from a web page and may not run here. */
  launchKind: "swf" | "page";
  libraryId: number | null;
}

export interface BrowsePage {
  total: number;
  pages: number;
  page: number;
  games: BrowseGame[];
}

export interface DownloadProgress {
  id: string;
  phase: "download" | "unpack" | "import" | "art" | "done";
  received: number;
  total: number;
}

export interface Downloaded {
  game: Game;
  already: boolean;
}

// ---------------------------------------------------------------- settings extras

export interface WatchReport {
  added: Game[];
  failed: { path: string; error: string }[];
  scanned: number;
}

export interface BackupProgress {
  phase: "pack" | "unpack";
  done: number;
  total: number;
}
