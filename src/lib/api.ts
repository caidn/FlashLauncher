// Everything the UI asks of the backend goes through here. In the Tauri app
// it's real commands; in a plain browser (npm run dev) it's an in-memory
// fake so the UI can still be worked on and previewed.

import { sampleCollections, sampleGames } from "../data/sample";
import type {
  ArtItem,
  ArtSlot,
  BackupProgress,
  BrowsePage,
  BrowseKind,
  BrowseSort,
  BulkReport,
  DownloadProgress,
  Downloaded,
  Collection,
  Game,
  GamePatch,
  ImportCandidate,
  ImportItem,
  ImportProgress,
  ImportSummary,
  ProjectorExit,
  ScrapeMatch,
  ScrapeProgress,
  ScrapeReport,
  SearchGroup,
  WatchReport,
} from "../data/types";
import { isTauri } from "./tauri";

export interface DropHandlers {
  enter: () => void;
  leave: () => void;
  drop: (paths: string[]) => void;
}

export interface Api {
  listGames(): Promise<Game[]>;
  listCollections(): Promise<Collection[]>;
  inspect(paths: string[]): Promise<ImportCandidate[]>;
  importGames(items: ImportItem[]): Promise<ImportSummary>;
  updateGame(id: number, patch: GamePatch): Promise<Game>;
  deleteGame(id: number): Promise<void>;
  getSettings(): Promise<Record<string, string>>;
  setSetting(key: string, value: string): Promise<void>;
  dataDir(): Promise<string>;
  pickFiles(): Promise<string[]>;
  pickFolder(): Promise<string | null>;
  onProgress(cb: (p: ImportProgress) => void): Promise<() => void>;
  onFileDrop(h: DropHandlers): Promise<() => void>;
  /** Turns an absolute file path from the backend into something <img> can load. */
  fileUrl(path: string): string;

  // player
  getGame(id: number): Promise<Game>;
  readSwf(id: number): Promise<ArrayBuffer>;
  getSaves(id: number): Promise<string | null>;
  putSaves(id: number, data: string): Promise<void>;
  sessionStart(id: number): Promise<number>;
  sessionUpdate(session: number, seconds: number): Promise<void>;
  listArt(id: number): Promise<ArtItem[]>;
  /** Selects an image for its slot (downloads it if remote); null clears `slot`. */
  selectArt(id: number, art: number | null, slot?: ArtSlot): Promise<void>;
  addScreenshot(id: number, png: Blob): Promise<ArtItem>;
  deleteArt(id: number, art: number): Promise<void>;
  uploadArt(id: number, slot: ArtSlot, path: string): Promise<void>;
  pickImage(): Promise<string | null>;

  // collections
  createCollection(name: string, games: number[]): Promise<Collection>;
  renameCollection(id: number, name: string): Promise<Collection>;
  deleteCollection(id: number): Promise<void>;
  addToCollection(id: number, games: number[]): Promise<Collection>;
  removeFromCollection(id: number, games: number[]): Promise<Collection>;
  reorderCollection(id: number, games: number[]): Promise<Collection>;

  // scraping
  scrapeSearch(id: number, title?: string): Promise<SearchGroup[]>;
  scrapeApply(id: number, picks: ScrapeMatch[]): Promise<ScrapeReport>;
  scrapeUnlink(id: number, source: string): Promise<void>;
  scrapeRefresh(id: number): Promise<ScrapeReport>;
  /** Auto-scrapes these games, or every game missing art when null. */
  scrapeBulk(ids: number[] | null): Promise<BulkReport>;
  scrapeCancel(): Promise<void>;
  onScrapeProgress(cb: (p: ScrapeProgress) => void): Promise<() => void>;
  launchProjector(id: number): Promise<void>;
  onProjectorExit(cb: (e: ProjectorExit) => void): Promise<() => void>;
  pickExe(): Promise<string | null>;
  setFullscreen(on: boolean): Promise<void>;
  /** Runs before the window closes, so the player can flush saves and play time. */
  onBeforeClose(cb: () => Promise<void>): Promise<() => void>;

  // flashpoint
  /** `page` is the API page to start reading from; the result's `page` is the last one read. */
  browse(query: string, page: number, sort: BrowseSort, kind: BrowseKind): Promise<BrowsePage>;
  downloadGame(id: string): Promise<Downloaded>;
  cancelDownload(): Promise<void>;
  onDownloadProgress(cb: (p: DownloadProgress) => void): Promise<() => void>;
  /** Base address a downloaded game's files are served from (ends with "/"). */
  contentBase(contentId: string): string;

  // settings extras
  /** Imports new SWFs from the watched folders (from settings). */
  scanWatched(): Promise<WatchReport>;
  applyPlayerDefaults(): Promise<void>;
  exportLibrary(path: string): Promise<number>;
  restoreLibrary(path: string): Promise<void>;
  onBackupProgress(cb: (p: BackupProgress) => void): Promise<() => void>;
  pickBackupTarget(defaultName: string): Promise<string | null>;
  pickBackup(): Promise<string | null>;
  /** Opens a folder in the file manager, or an https link in the browser. */
  openExternal(target: string): Promise<void>;
  reloadApp(): void;
  /** Takes the game off your Discord profile (setting it happens when a session starts). */
  discordClear(): Promise<void>;
}

// ---------------------------------------------------------------- tauri

let convert: ((p: string) => string) | null = null;

const tauriBase = {
  async listGames() {
    return invoke("library_list");
  },
  async listCollections() {
    return invoke("collections_list");
  },
  async inspect(paths) {
    return invoke("library_inspect", { paths });
  },
  async importGames(items) {
    return invoke("library_import", { items });
  },
  async updateGame(id, patch) {
    return invoke("game_update", { id, patch });
  },
  async deleteGame(id) {
    return invoke("game_delete", { id });
  },
  async getSettings() {
    return invoke("settings_get");
  },
  async setSetting(key, value) {
    return invoke("settings_set", { key, value });
  },
  async dataDir() {
    return invoke("app_data_dir");
  },
  async pickFiles() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({ multiple: true, title: "Add SWF files", filters: [{ name: "Flash", extensions: ["swf"] }] });
    if (!res) return [];
    return Array.isArray(res) ? res : [res];
  },
  async pickFolder() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({ directory: true, title: "Scan a folder for SWF files" });
    return typeof res === "string" ? res : null;
  },
  async onProgress(cb) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ImportProgress>("import://progress", (e) => cb(e.payload));
  },
  async onFileDrop(h) {
    const { getCurrentWebview } = await import("@tauri-apps/api/webview");
    return getCurrentWebview().onDragDropEvent((e) => {
      const p = e.payload;
      if (p.type === "enter" || p.type === "over") h.enter();
      else if (p.type === "leave") h.leave();
      else if (p.type === "drop") h.drop(p.paths);
    });
  },
  fileUrl(path) {
    return convert ? convert(path) : path;
  },
  async getGame(id) {
    return invoke("game_get", { id });
  },
  async readSwf(id) {
    return invoke("game_read_swf", { id });
  },
  async getSaves(id) {
    return invoke("saves_get", { id });
  },
  async putSaves(id, data) {
    return invoke("saves_put", { id, data });
  },
  async sessionStart(id) {
    return invoke("session_start", { id });
  },
  async sessionUpdate(session, seconds) {
    return invoke("session_update", { session, seconds });
  },
  async listArt(id) {
    return invoke("art_list", { id });
  },
  async selectArt(id, art, slot) {
    return invoke("art_select", { id, art, slot: slot ?? null });
  },
  async addScreenshot(id, png) {
    const core = await import("@tauri-apps/api/core");
    const bytes = new Uint8Array(await png.arrayBuffer());
    return core.invoke<ArtItem>("art_add_screenshot", bytes, { headers: { "x-game-id": String(id) } });
  },
  async deleteArt(id, art) {
    return invoke("art_delete", { id, art });
  },
  async uploadArt(id, slot, path) {
    return invoke("art_upload", { id, slot, path });
  },
  async pickImage() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({
      title: "Pick an image",
      filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "gif", "webp"] }],
    });
    return typeof res === "string" ? res : null;
  },
  async createCollection(name, games) {
    return invoke("collection_create", { name, games });
  },
  async renameCollection(id, name) {
    return invoke("collection_rename", { id, name });
  },
  async deleteCollection(id) {
    return invoke("collection_delete", { id });
  },
  async addToCollection(id, games) {
    return invoke("collection_add", { id, games });
  },
  async removeFromCollection(id, games) {
    return invoke("collection_remove", { id, games });
  },
  async reorderCollection(id, games) {
    return invoke("collection_reorder", { id, games });
  },
  async scrapeSearch(id, title) {
    return invoke("scrape_search", { id, title: title ?? null });
  },
  async scrapeApply(id, picks) {
    return invoke("scrape_apply", { id, picks });
  },
  async scrapeUnlink(id, source) {
    return invoke("scrape_unlink", { id, source });
  },
  async scrapeRefresh(id) {
    return invoke("scrape_refresh", { id });
  },
  async scrapeBulk(ids) {
    return invoke("scrape_bulk", { ids });
  },
  async scrapeCancel() {
    return invoke("scrape_cancel");
  },
  async onScrapeProgress(cb) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ScrapeProgress>("scrape://progress", (e) => cb(e.payload));
  },
  async launchProjector(id) {
    return invoke("projector_launch", { id });
  },
  async onProjectorExit(cb) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<ProjectorExit>("projector://exit", (e) => cb(e.payload));
  },
  async pickExe() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({
      title: "Find the Flash Player projector",
      filters: [{ name: "Programs", extensions: ["exe", "app", "*"] }],
    });
    return typeof res === "string" ? res : null;
  },
  async setFullscreen(on) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().setFullscreen(on);
  },
  async onBeforeClose(cb) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    // Tauri waits for this handler before closing the window.
    return getCurrentWindow().onCloseRequested(async () => {
      await Promise.race([cb(), new Promise((r) => setTimeout(r, 1500))]);
    });
  },
} satisfies Partial<Api>;

const tauriExtra = {
  async browse(query: string, page: number, sort: BrowseSort, kind: BrowseKind) {
    return invoke("flashpoint_browse", { query, page, sort, kind });
  },
  async downloadGame(id: string) {
    return invoke("flashpoint_download", { id });
  },
  async cancelDownload() {
    return invoke("flashpoint_cancel");
  },
  async onDownloadProgress(cb: (p: DownloadProgress) => void) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<DownloadProgress>("flashpoint://progress", (e) => cb(e.payload));
  },
  contentBase(contentId: string) {
    // Same address shape Tauri uses for custom protocols on each platform.
    const windows = /windows/i.test(navigator.userAgent);
    return windows ? `http://fpcontent.localhost/${contentId}/` : `fpcontent://localhost/${contentId}/`;
  },
  async scanWatched() {
    return invoke("watch_scan");
  },
  async applyPlayerDefaults() {
    return invoke("games_apply_defaults");
  },
  async exportLibrary(path: string) {
    return invoke("library_export", { path });
  },
  async restoreLibrary(path: string) {
    return invoke("library_restore", { path });
  },
  async onBackupProgress(cb: (p: BackupProgress) => void) {
    const { listen } = await import("@tauri-apps/api/event");
    return listen<BackupProgress>("backup://progress", (e) => cb(e.payload));
  },
  async pickBackupTarget(defaultName: string) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const res = await save({ title: "Back up your library", defaultPath: defaultName, filters: [{ name: "Flash Launcher backup", extensions: ["zip"] }] });
    return typeof res === "string" ? res : null;
  },
  async pickBackup() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const res = await open({ title: "Restore a backup", filters: [{ name: "Flash Launcher backup", extensions: ["zip"] }] });
    return typeof res === "string" ? res : null;
  },
  async openExternal(target: string) {
    return invoke("open_external", { target });
  },
  async discordClear() {
    return invoke("discord_clear");
  },
  reloadApp() {
    window.location.reload();
  },
} satisfies Partial<Api>;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const core = await import("@tauri-apps/api/core");
  return core.invoke<T>(cmd, args);
}

if (isTauri) {
  import("@tauri-apps/api/core").then((core) => (convert = core.convertFileSrc));
}

// ---------------------------------------------------------------- browser fake

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
// ?empty in the URL starts the preview with an empty library.
let fakeGames = typeof location !== "undefined" && location.search.includes("empty") ? [] : sampleGames();
let fakeCollections = sampleCollections();
const fakeSettings: Record<string, string> = {};
const fakeSaves = new Map<number, string>();
const fakeArt = new Map<number, ArtItem[]>();
let fakeArtId = 1;
let progressCb: ((p: ImportProgress) => void) | null = null;
let scrapeCb: ((p: ScrapeProgress) => void) | null = null;
let fakeCancel = false;

function fakeCandidate(path: string, i: number): ImportCandidate {
  const fileName = path.split(/[\\/]/).pop() ?? path;
  // Rough stand-in for the Rust title cleanup (src-tauri/src/title.rs).
  const title = fileName
    .replace(/\.swf$/i, "")
    .replace(/(^|[_\-. ])v?\d+([._]\d+)+(?=$|[_\-. ])/gi, " ")
    .replace(/(^|[_\-. ])v\d+(?=$|[_\-. ])/gi, " ")
    .split(/[_\-. ]+/)
    .filter((w) => w && !["ng", "final", "secure", "kongregate"].includes(w.toLowerCase()))
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join(" ");
  const bad = /broken/i.test(fileName);
  const dupe = fakeGames.find((g) => g.title.toLowerCase() === title.toLowerCase());
  return {
    path,
    fileName,
    fileSize: 300_000 + i * 211_337,
    sha256: bad ? null : `fake${i}`.padEnd(64, "0"),
    title,
    info: bad
      ? null
      : {
          compression: i % 2 ? "zlib" : "lzma",
          version: 9 + i,
          uncompressedLen: 1_000_000,
          width: 550 + i * 90,
          height: 400 + i * 50,
          frameRate: [24, 30, 60][i % 3],
          frameCount: 1,
          asVersion: (i % 3 === 0 ? 2 : 3) as 2 | 3,
          usesNetwork: false,
          background: "#000000",
          metaTitle: null,
        },
    error: bad ? "not a SWF file (bad signature)" : null,
    duplicateOf: dupe ? { id: dupe.id, title: dupe.title } : null,
    duplicateInBatch: false,
    preview: null,
    imageCount: bad ? 0 : i % 3,
  };
}

const browserBase = {
  async listGames() {
    return fakeGames.map((g) => ({ ...g }));
  },
  async listCollections() {
    return fakeCollections.map((c) => ({ ...c, gameIds: c.gameIds.filter((id) => fakeGames.some((g) => g.id === id)) }));
  },
  async inspect(paths) {
    const out: ImportCandidate[] = [];
    for (let i = 0; i < paths.length; i++) {
      progressCb?.({ phase: "inspect", done: i, total: paths.length, current: paths[i] });
      await wait(250);
      out.push(fakeCandidate(paths[i], i));
    }
    return out;
  },
  async importGames(items) {
    const imported: Game[] = [];
    const palette = ["#00aba9", "#d80073", "#60a917", "#fa6800", "#6a00ff"];
    for (let i = 0; i < items.length; i++) {
      progressCb?.({ phase: "import", done: i, total: items.length, current: items[i].title });
      await wait(300);
      const id = Math.max(0, ...fakeGames.map((g) => g.id)) + 1;
      const base = sampleGames()[0];
      const g: Game = {
        ...base,
        id,
        title: items[i].title,
        developer: null,
        year: null,
        favorite: false,
        playSeconds: 0,
        lastPlayed: null,
        addedAt: new Date().toISOString(),
        color: palette[id % palette.length],
        tags: [],
        compat: "unknown",
        originalName: items[i].path.split(/[\\/]/).pop() ?? "",
      };
      fakeGames.push(g);
      imported.push(g);
    }
    return { imported, failed: [] };
  },
  async updateGame(id, patch) {
    const g = fakeGames.find((x) => x.id === id);
    if (!g) throw new Error(`game ${id} not found`);
    Object.assign(g, patch);
    return { ...g };
  },
  async deleteGame(id) {
    fakeGames = fakeGames.filter((g) => g.id !== id);
  },
  async getSettings() {
    return { ...fakeSettings };
  },
  async setSetting(key, value) {
    fakeSettings[key] = value;
  },
  async dataDir() {
    return "(browser preview, nothing is saved)";
  },
  async pickFiles() {
    return ["C:/Downloads/paper_plane_derby_v2.swf", "C:/Downloads/ng_skyline_bandit_final.swf", "C:/Downloads/broken_upload.swf"];
  },
  async pickFolder() {
    return null;
  },
  async onProgress(cb) {
    progressCb = cb;
    return () => {
      if (progressCb === cb) progressCb = null;
    };
  },
  async onFileDrop(h) {
    // HTML5 drag events don't expose real paths; file names stand in.
    let depth = 0;
    const over = (e: DragEvent) => e.preventDefault();
    const enter = (e: DragEvent) => {
      e.preventDefault();
      if (depth++ === 0) h.enter();
    };
    const leave = () => {
      if (--depth <= 0) {
        depth = 0;
        h.leave();
      }
    };
    const drop = (e: DragEvent) => {
      e.preventDefault();
      depth = 0;
      h.drop(Array.from(e.dataTransfer?.files ?? []).map((f) => `C:/Dropped/${f.name}`));
    };
    window.addEventListener("dragover", over);
    window.addEventListener("dragenter", enter);
    window.addEventListener("dragleave", leave);
    window.addEventListener("drop", drop);
    return () => {
      window.removeEventListener("dragover", over);
      window.removeEventListener("dragenter", enter);
      window.removeEventListener("dragleave", leave);
      window.removeEventListener("drop", drop);
    };
  },
  fileUrl(path) {
    return path;
  },
  async getGame(id) {
    const g = fakeGames.find((x) => x.id === id);
    if (!g) throw new Error(`game ${id} not found`);
    return { ...g };
  },
  async readSwf(id) {
    // The preview has no real files. Drop SWFs into public/dev-swfs/ named
    // 1.swf, 2.swf, ... (by game id) to try the player in a browser.
    const res = await fetch(`/dev-swfs/${id}.swf`);
    const type = res.headers.get("content-type") ?? "";
    if (!res.ok || type.includes("html")) {
      throw new Error("There's no real SWF behind the sample games in the browser preview. Run the desktop app to play.");
    }
    return res.arrayBuffer();
  },
  async getSaves(id) {
    return fakeSaves.get(id) ?? null;
  },
  async putSaves(id, data) {
    fakeSaves.set(id, data);
  },
  async sessionStart(id) {
    const g = fakeGames.find((x) => x.id === id);
    if (g) g.lastPlayed = new Date().toISOString();
    return id * 1000;
  },
  async sessionUpdate() {},
  async listArt(id) {
    return (fakeArt.get(id) ?? []).map((a) => ({ ...a }));
  },
  async selectArt(id, art, slot) {
    const items = fakeArt.get(id) ?? [];
    const target = items.find((a) => a.id === art);
    const s = target?.slot ?? slot;
    items.forEach((a) => {
      if (a.slot === s) a.selected = a.id === art;
    });
    const g = fakeGames.find((x) => x.id === id);
    if (g && s) {
      g.art = { ...g.art, [s]: target ? target.path || target.remoteUrl : null };
      g.fallbackImage = g.art.embedded ?? g.art.screenshot ?? null;
    }
  },
  async addScreenshot(id, png) {
    const bmp = await createImageBitmap(png);
    const item: ArtItem = {
      id: fakeArtId++,
      slot: "screenshot",
      source: "player",
      path: URL.createObjectURL(png),
      remoteUrl: null,
      thumbUrl: null,
      width: bmp.width,
      height: bmp.height,
      selected: false,
    };
    fakeArt.set(id, [item, ...(fakeArt.get(id) ?? [])]);
    return { ...item };
  },
  async deleteArt(id, art) {
    fakeArt.set(id, (fakeArt.get(id) ?? []).filter((a) => a.id !== art));
  },
  async uploadArt() {
    throw new Error("Uploading art only works in the desktop app.");
  },
  async pickImage() {
    return null;
  },
  async createCollection(name, games) {
    if (!name.trim()) throw new Error("a collection needs a name");
    const c = { id: Math.max(0, ...fakeCollections.map((x) => x.id)) + 1, name: name.trim(), gameIds: [...new Set(games)] };
    fakeCollections.push(c);
    return { ...c };
  },
  async renameCollection(id, name) {
    const c = fakeCollections.find((x) => x.id === id);
    if (!c) throw new Error("not found");
    c.name = name.trim();
    return { ...c };
  },
  async deleteCollection(id) {
    fakeCollections = fakeCollections.filter((x) => x.id !== id);
  },
  async addToCollection(id, games) {
    const c = fakeCollections.find((x) => x.id === id)!;
    for (const g of games) if (!c.gameIds.includes(g)) c.gameIds.push(g);
    return { ...c, gameIds: [...c.gameIds] };
  },
  async removeFromCollection(id, games) {
    const c = fakeCollections.find((x) => x.id === id)!;
    c.gameIds = c.gameIds.filter((g) => !games.includes(g));
    return { ...c, gameIds: [...c.gameIds] };
  },
  async reorderCollection(id, games) {
    const c = fakeCollections.find((x) => x.id === id)!;
    c.gameIds = games.filter((g) => c.gameIds.includes(g));
    return { ...c, gameIds: [...c.gameIds] };
  },
  async scrapeSearch(id, title) {
    await wait(500);
    const g = fakeGames.find((x) => x.id === id);
    const t = title || g?.title || "";
    const mk = (source: string, n: number, score: number, extra: Partial<ScrapeMatch> = {}): ScrapeMatch => ({
      source,
      id: `${source}-${n}`,
      title: n === 0 ? t : `${t} ${n + 1}`,
      altTitles: [],
      developer: g?.developer ?? null,
      year: g?.year ?? null,
      platform: "Flash",
      thumb: null,
      score,
      fileMatch: false,
      ...extra,
    });
    return [
      { source: "flashpoint", label: "Flashpoint Archive", unavailable: null, error: null, linkedId: null,
        matches: [mk("flashpoint", 0, 1, { fileMatch: true }), mk("flashpoint", 1, 0.62)] },
      { source: "steamgriddb", label: "SteamGridDB", unavailable: "add a SteamGridDB API key in settings", error: null, linkedId: null, matches: [] },
      { source: "igdb", label: "IGDB", unavailable: "add an IGDB client ID and secret in settings", error: null, linkedId: null, matches: [] },
      { source: "archive", label: "Internet Archive", unavailable: null, error: null, linkedId: null,
        matches: [mk("archive", 0, 0.97, { platform: null })] },
    ];
  },
  async scrapeApply(id, picks) {
    await wait(400);
    const g = fakeGames.find((x) => x.id === id);
    if (g) {
      g.links = picks.map((p) => p.source);
      g.scrapedAt = new Date().toISOString();
    }
    return { gameId: id, title: g?.title ?? "", linked: g?.links ?? [], needsReview: false, errors: [] };
  },
  async scrapeUnlink(id, source) {
    const g = fakeGames.find((x) => x.id === id);
    if (g) g.links = g.links.filter((l) => l !== source);
  },
  async scrapeRefresh(id) {
    const g = fakeGames.find((x) => x.id === id);
    return { gameId: id, title: g?.title ?? "", linked: g?.links ?? [], needsReview: false, errors: [] };
  },
  async scrapeBulk(ids) {
    const list = ids ?? fakeGames.filter((g) => !g.scrapedAt).map((g) => g.id);
    const reports: ScrapeReport[] = [];
    for (let i = 0; i < list.length; i++) {
      if (fakeCancel) break;
      const g = fakeGames.find((x) => x.id === list[i]);
      scrapeCb?.({ done: i, total: list.length, current: g?.title ?? "", finished: false });
      await wait(450);
      if (g) {
        g.scrapedAt = new Date().toISOString();
        if (i % 4 !== 3) g.links = ["flashpoint"];
      }
      reports.push({ gameId: list[i], title: g?.title ?? "", linked: g?.links ?? [], needsReview: i % 4 === 3, errors: [] });
    }
    const cancelled = fakeCancel;
    fakeCancel = false;
    scrapeCb?.({ done: list.length, total: list.length, current: "", finished: true });
    return { reports, cancelled };
  },
  async scrapeCancel() {
    fakeCancel = true;
  },
  async onScrapeProgress(cb) {
    scrapeCb = cb;
    return () => {
      if (scrapeCb === cb) scrapeCb = null;
    };
  },
  async launchProjector() {
    throw new Error("The projector only works in the desktop app.");
  },
  async onProjectorExit() {
    return () => {};
  },
  async pickExe() {
    return "C:/Flash/flashplayer_32_sa.exe";
  },
  async setFullscreen(on) {
    try {
      if (on) await document.documentElement.requestFullscreen();
      else if (document.fullscreenElement) await document.exitFullscreen();
    } catch {
      /* not allowed without a user gesture; fine for a preview */
    }
  },
  async onBeforeClose() {
    return () => {};
  },
} satisfies Partial<Api>;

// A few made-up Flashpoint entries so the browse page can be worked on.
const FAKE_FP: [string, string, number][] = [
  ["Sky Ferry", "Little Owl Games", 2008],
  ["Gravity Goo", "Blobworks", 2010],
  ["Paper Plane Derby", "Foldable", 2006],
  ["Tower of Toast", "Crumb Studio", 2012],
  ["Neon Drift", "Overdrive", 2009],
  ["Moon Miner", "Crater Co", 2011],
  ["Castle Kitchen", "Ladle Interactive", 2013],
  ["Snowball Siege", "Frostbite Flash", 2007],
  ["Robot Garden", "Bolt & Bloom", 2014],
  ["Deep Sea Diner", "Kelp Games", 2010],
  ["Pixel Pirates", "8-Bit Cove", 2005],
  ["Hot Air Hopper", "Balloonatic", 2012],
];
let fakeDownloadCb: ((p: DownloadProgress) => void) | null = null;
let fakeDownloadCancel = false;
const fakeLinks = new Map<string, number>();

const browserExtra = {
  async browse(query: string, page: number, sort: BrowseSort, kind: BrowseKind) {
    await wait(350);
    if (kind === "animations") return { total: 0, pages: 0, page, games: [] };
    const q = query.trim().toLowerCase();
    let list = FAKE_FP.map(([title, developer, year], i) => ({
      id: `00000000-0000-0000-0000-${String(i).padStart(12, "0")}`,
      title,
      developer,
      publisher: null,
      series: null,
      releaseDate: `${year}-03-0${(i % 9) + 1}`,
      year,
      description: `${title} is a made-up game for previewing the browse page. In the desktop app this is Flashpoint's own description.`,
      tags: ["Arcade", "Puzzle", "Platformer"].slice(0, (i % 3) + 1),
      playMode: "Single Player",
      language: "en",
      logo: null,
      logoThumb: null,
      screenshot: null,
      size: i % 4 === 3 ? null : 400_000 + i * 913_377,
      launchKind: (i === 6 ? "page" : "swf") as "swf" | "page",
      libraryId: fakeLinks.get(`00000000-0000-0000-0000-${String(i).padStart(12, "0")}`) ?? null,
    })).filter((g) => !q || g.title.toLowerCase().includes(q));
    if (sort === "newest") list = list.sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
    if (sort === "oldest") list = list.sort((a, b) => (a.year ?? 0) - (b.year ?? 0));
    if (sort === "title") list = list.sort((a, b) => a.title.localeCompare(b.title));
    const per = 8;
    return { total: list.length, pages: Math.ceil(list.length / per), page, games: list.slice((page - 1) * per, page * per) };
  },
  async downloadGame(id: string) {
    fakeDownloadCancel = false;
    const all = [...(await browserExtra.browse("", 1, "title", "games")).games, ...(await browserExtra.browse("", 2, "title", "games")).games];
    const entry = all.find((g) => g.id === id)!;
    const total = entry.size ?? 2_000_000;
    for (let r = 0; r <= total; r += total / 20) {
      if (fakeDownloadCancel) throw new Error("download cancelled");
      fakeDownloadCb?.({ id, phase: "download", received: r, total });
      await wait(90);
    }
    for (const phase of ["unpack", "import", "art"] as const) {
      fakeDownloadCb?.({ id, phase, received: 0, total: 0 });
      await wait(300);
    }
    const base = sampleGames()[0];
    const gid = Math.max(0, ...fakeGames.map((g) => g.id)) + 1;
    const game: Game = {
      ...base,
      id: gid,
      title: entry.title,
      developer: entry.developer,
      year: entry.year,
      description: entry.description,
      source: "archive",
      color: "#1ba1e2",
      playSeconds: 0,
      lastPlayed: null,
      addedAt: new Date().toISOString(),
      favorite: false,
      tags: entry.tags.map((t: string) => t.toLowerCase()),
      contentId: id,
      launchUrl: `http://example.com/${entry.title.replace(/\W/g, "").toLowerCase()}.swf`,
      links: ["flashpoint"],
      scrapedAt: new Date().toISOString(),
    };
    fakeGames.push(game);
    fakeLinks.set(id, gid);
    fakeDownloadCb?.({ id, phase: "done", received: 1, total: 1 });
    return { game, already: false };
  },
  async cancelDownload() {
    fakeDownloadCancel = true;
  },
  async onDownloadProgress(cb: (p: DownloadProgress) => void) {
    fakeDownloadCb = cb;
    return () => {
      if (fakeDownloadCb === cb) fakeDownloadCb = null;
    };
  },
  contentBase(contentId: string) {
    return `/dev-content/${contentId}/`;
  },
  async scanWatched() {
    await wait(500);
    return { added: [], failed: [], scanned: 0 };
  },
  async applyPlayerDefaults() {
    const scale = fakeSettings.defaultScaleMode;
    const quality = fakeSettings.defaultQuality;
    for (const g of fakeGames) {
      if (scale) g.scaleMode = scale as Game["scaleMode"];
      if (quality) g.quality = quality as Game["quality"];
    }
  },
  async exportLibrary() {
    await wait(800);
    return 12_345_678;
  },
  async restoreLibrary() {
    await wait(500);
    throw new Error("Restoring only works in the desktop app.");
  },
  async onBackupProgress() {
    return () => {};
  },
  async pickBackupTarget(name: string) {
    return `C:/Users/you/Documents/${name}`;
  },
  async pickBackup() {
    return "C:/Users/you/Documents/flash-launcher-backup.zip";
  },
  async openExternal(target: string) {
    if (target.startsWith("https://")) window.open(target, "_blank");
  },
  async discordClear() {},
  reloadApp() {
    window.location.reload();
  },
} satisfies Partial<Api>;

const tauriApi: Api = { ...tauriBase, ...tauriExtra };
const browserApi: Api = { ...browserBase, ...browserExtra };

export const api: Api = isTauri ? tauriApi : browserApi;
