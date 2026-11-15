# Flash Launcher

A desktop launcher for archived Flash games and your own SWFs, styled after Windows 8 / Windows Phone Metro. Built with Tauri 2, React and TypeScript. Flash runs through [Ruffle](https://ruffle.rs).

Status: all 6 build steps done. Shell, design system, library (import, header parsing, embedded art, SQLite), player (Ruffle, in-game menu, saves, play time, screenshots, projector), scraping (Flashpoint Archive, SteamGridDB, IGDB, Internet Archive), game pages, collections, live-tile home screen, downloading games from Flashpoint Archive, controller support, watched folders, player defaults, and library backup/restore. Discord Rich Presence shows what you're playing.

## Running it

You need [Node 20+](https://nodejs.org), [Rust](https://rustup.rs), and on Windows the WebView2 runtime (already on Windows 10/11) plus the MSVC build tools. See [Tauri's prerequisites](https://v2.tauri.app/start/prerequisites/) if anything's missing.

```
npm install
npm run tauri dev
```

The first `tauri dev` compiles the Rust side and takes a few minutes. After that it's fast.

Ruffle comes from npm (`@ruffle-rs/ruffle`, pinned to a nightly in package.json) and is copied into `public/ruffle/` automatically before `dev` and `build`. To update it: `npm install --save-exact @ruffle-rs/ruffle@<version>`.

To work on just the UI in a browser (no window controls, no native features):

```
npm run dev
```

then open http://localhost:1420. In the browser there's no Rust backend, so the UI runs against a fake in-memory library of made-up games (`src/lib/api.ts`). Add `?empty` to the URL to see the first-launch state. To try the player in the browser, put SWFs in `public/dev-swfs/` named by sample game id (`1.swf`, `2.swf`, ...).

## Playing

Click a game to open its page: hero banner, logo, play button, play time, description, screenshots, tags, details and collections. The page grows out of the tile you clicked. Esc or Backspace goes back.

Right-click selects games instead (the bottom bar then has play, find info, art, collections, favorite, rename, delete).

Collections: make them on the collections page or from any game's page. Inside a collection, right-click games to select and remove them.

In a game: `Esc` or `F1` opens the menu, `F11` toggles full screen, `F12` takes a screenshot. The menu has volume, scale (fit / stretch / original), the color behind the game, quality, frame rate, Ruffle vs. projector, and whether the game may use the internet (off by default).

Saves: Ruffle keeps Flash saves in the WebView's localStorage. The launcher copies each game's saves into the library DB while it runs and on exit, and loads only that game's saves before it starts, so games can't see each other's data. Deleting a game keeps its saves aside by file hash, and re-importing the same SWF brings them back.

Controller: d-pad or left stick moves around the launcher, A selects, B goes back, Y is the right-click action (select in the library), LB / RB switch between home, library, collections and browse. In a game, the controller presses keys: each game has a **controller** button on its page to pick the layout (arrows + space, WASD, or your own). Hold View + Menu together (or the guide button) to open the in-game menu.

Projector: for AS3 games Ruffle can't run yet, set the path to your own standalone Flash Player in settings, then switch the game to "flash projector" in its menu. Play time is tracked until the projector closes.

## Downloading from Flashpoint Archive

The **browse** page searches Flashpoint Archive's Flash collection (games by default, animations on the other tab). Open an entry, hit **download**, and it's added to your library with its title, description, tags, title card and screenshot already filled in. Downloads run one at a time with a strip along the bottom, so you can keep browsing.

How it works: Flashpoint keeps every file a game ever loaded, laid out by the address it lived at (`content/<host>/<path>` in each game's zip on `download.flashpointarchive.org`). The launcher checks the zip against Flashpoint's SHA-256, unpacks it to `flashpoint/<id>/`, and imports the main SWF like any other game. When the game runs, Ruffle loads it from its original address and every http(s) request is rewritten to `fpcontent://` (a small protocol the backend serves from that folder), so levels, music and XML resolve offline. Older "legacy" entries have no zip; their SWF comes from Flashpoint's htdocs mirror and anything else the game asks for is fetched on first use and kept.

Games that start from a web page instead of a SWF are flagged in the dialog; the launcher pulls out the SWF the page embeds, which works for most but not all.

`cargo test live_download -- --ignored --nocapture` downloads QWOP (zip) and Fancy Snowboarding (legacy) for real.

## Discord

While a game runs, your Discord profile shows "Playing Flash Launcher" with the game's title, developer and year, an elapsed timer, and its art (any selected art that came from the web, since Discord loads the image itself). Games with only local art show the `flash_launcher` image uploaded to the Discord app's Rich Presence art assets. It talks to the Discord desktop app over its local IPC pipe (`src-tauri/src/discord.rs`), so nothing happens if Discord isn't running, and it reconnects on its own if Discord starts later. Turn it off in settings under player.

## Settings worth knowing

- **New games start with**: default scaling, quality and internet access for imports and downloads, plus a button to apply them to every game.
- **Watched folders**: SWFs saved into these folders are imported on their own (checked at startup and when the window regains focus, at most once a minute). Files are remembered by hash, so a game you delete stays deleted.
- **Back up library**: one zip with the database (games, saves, play time, collections, settings including API keys), SWFs, art and downloads. **Restore** swaps the whole library for a backup and keeps the old one in `before-restore/`.

## Tests

```
cd src-tauri
cargo test
```

covers the SWF parser (all three compression types, tag walking, AS version, metadata), image extraction (every bitmap tag type), title cleanup, and a full inspect, import, edit, delete round trip against a throwaway database.

To run the parser over a folder of real SWFs and print what it finds:

```
SWF_SAMPLES=/path/to/swfs cargo test real_files -- --ignored --nocapture
```

## Art and info

New games are looked up automatically after import. Each source is searched, the results are scored against the game's title (and the original SWF filename, which Flashpoint knows for most entries), and only clear matches are used. Anything uncertain shows up in the library as "wasn't matched automatically" with a review button.

Right-click a game for **find info** (pick the right entry in each source yourself) or **art** (choose the image for each slot: cover, wide, hero, logo, icon, screenshot, or a picture from inside the SWF, or upload your own). Settings has **find missing art for all games**.

| Source | Needs | Gives |
| --- | --- | --- |
| Flashpoint Archive | nothing | proper titles, developer, publisher, release date, description, tags, title card, screenshot |
| SteamGridDB | free API key (steamgriddb.com → profile → preferences → API) | covers, wide grids, heroes, transparent logos, icons |
| IGDB | Twitch app client ID + secret (dev.twitch.tv/console/apps) | details and covers for bigger commercial games, artwork, screenshots |
| Internet Archive | nothing | last-resort details and a thumbnail |

Renaming a game locks its title, so later lookups won't overwrite it. Downloaded images are scaled to sensible sizes and cached, so nothing is fetched twice.

### Adding a scraper source

1. Add a module in `src-tauri/src/scrape/` with a struct that implements `Scraper` (`search`, `details`, `art`, plus `id`, `label`, `metadata_priority` and `unavailable` for missing keys).
2. Put whatever `details`/`art` need into `Match.payload` during `search` if the source returns it anyway; it's stored with the link so later refreshes don't need another lookup.
3. Register it in `all()` in `scrape/mod.rs`, add its preference in `slot_preference` (`scrape/service.rs`) for the art slots it provides, and give it a polite gap in `http.rs`.
4. Write a parser test against a trimmed real response, like the ones in `flashpoint.rs`.

The UI picks new sources up automatically: the match dialog lists every source `all()` returns.

`cargo test live_auto_scrape -- --ignored --nocapture` runs a real lookup against Flashpoint and the Internet Archive.

## Where your stuff lives

Everything is under the app data folder (`%APPDATA%\com.flashlauncher.app` on Windows):

```
library.db          SQLite: games, art, collections, play sessions, settings
swfs/<sha256>.swf   the library's own copy of each game
art/<game id>/      images from the SWF, screenshots, uploads, downloaded art
cache/remote/       downloaded images, by URL, so nothing is fetched twice
cache/pending/      images from imports you haven't confirmed yet, cleared on launch
cache/downloads/    partial Flashpoint downloads, cleared on launch
flashpoint/<id>/    downloaded games' own files, as <host>/<path>
before-restore/     your previous library, after restoring a backup
```

Importing never touches your original files.

## Layout

```
src/
  styles/        tokens (type, colors, motion curves), global styles, fonts
  theme/         accent palette and the theme provider (accent, light/dark, reduced motion)
  components/    TitleBar, Pivot (main nav), Tile (live tiles), Hub (panorama), FallbackArt, Controls
  pages/         Home, Library, Collections, AddGame, Settings
  lib/           spatial (arrow key) navigation, tilt-on-press, Tauri wrapper, formatting
  data/          types, LibraryProvider (games + collections state), browser sample data
  import/        import flow: whole-window drop target, review dialog, progress
  player/        Ruffle loader, player view, in-game menu, save syncing
  art/           match dialog, art picker, background bulk scraping
  pages/Game/    the game page (continuum transition from the clicked tile)
  pages/Collections/  collection list and single-collection view
  pages/Browse/  Flashpoint search, entry dialog
  browse/        background download queue and progress strip
  lib/gamepad.ts controller navigation for the launcher's own screens
  player/gamepadMap.ts, ControllerDialog  per-game controller -> key layouts
src-tauri/src/
  swf/           SWF reader: header, zlib/LZMA, tags, embedded bitmaps
  title.rs       filename -> title cleanup, title normalizing
  import.rs      inspect (hash, parse, flag duplicates) and import (copy, insert, art)
  library.rs     game/collection/settings queries
  player.rs      SWF bytes, saves, play sessions, screenshots, projector
  scrape/        scraper trait, HTTP client (rate limits, retries, image cache),
                 Flashpoint / SteamGridDB / IGDB / Internet Archive, and the
                 service that links matches, applies metadata and picks art
  download.rs    Flashpoint browse + download (zip or legacy), main SWF detection
  content.rs     the fpcontent:// protocol that serves downloaded games' files
  watch.rs       watched-folder scanning
  backup.rs      library backup zip and restore
  discord.rs     Discord Rich Presence over the local IPC pipe
  db.rs          schema and migrations (PRAGMA user_version)
  commands.rs    the Tauri commands the UI calls
```

## Design rules

- Zero border radius, no shadows, no gradients, no blur. Solid fills.
- Type does the work: big light lowercase headers, Segoe UI on Windows with Selawik (Microsoft's open-source Segoe stand-in, OFL licensed) as the fallback.
- One accent color drives focus rings, selected states and art-less tiles.
- Motion uses the Windows 8 curve `cubic-bezier(0.1, 0.9, 0.2, 1)`, 150 to 350ms. Everything respects reduced motion.
- Anything focusable that should be reachable with arrow keys or a gamepad gets `data-nav`.
- Dialogs are full-width bands (`BandDialog`), selection actions live in a bottom app bar (`AppBar`), notifications are accent toasts in the top right. Reuse these rather than inventing new patterns.
