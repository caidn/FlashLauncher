# Build: Flash Launcher

Build a desktop launcher for playing archived Flash games and user-uploaded SWF files. Think Steam or Xbox library, but with a sharp Windows 8 / Windows Phone "Frutiger Metro" look. This is a new project in an empty repo, so set up the full structure.

## Stack
- Tauri 2 + React + TypeScript + Vite. Use Electron only if Tauri blocks something specific, and tell me why.
- SQLite (via tauri-plugin-sql or rusqlite) for the library database.
- Ruffle (the self-hosted web build, latest release) as the main Flash runtime, running inside the app webview.
- Optional fallback: if the user points the app at their own standalone Flash Player projector exe in settings, games can be set to "launch with projector" for AS3 titles that Ruffle can't run yet. Never bundle Adobe binaries.
- Plain CSS modules or vanilla-extract for styling. No Tailwind component kits, no Material, no shadcn. The look has to be custom.

## Core features

### 1. Library
- Grid and list views of every game, with sorting (recent, A-Z, most played, date added) and filters (source: uploaded / archive, ActionScript version, favorites, tags).
- Collections the user can make, like Steam collections.
- Per-game page: hero banner across the top, logo over the hero, a play button, play time, last played, developer/publisher, year, description, screenshots, tags, and a "compatibility" badge (Ruffle works / partial / needs projector).
- Track play sessions and total play time.
- Search across the whole library with instant results.

### 2. SWF uploads
- Drag and drop SWFs anywhere in the window, or use an "add game" button. Support adding several at once.
- Copy imported files into the app's data folder. Hash each with SHA-256 so duplicates get caught.
- Parse the SWF header on import (in Rust): compression type (FWS/CWS/ZWS), SWF version, stage size, frame rate, frame count, and whether it uses AS3 (check for the FileAttributes tag's AS3 flag / DoABC tags).
- Pull embedded images out of the SWF (DefineBitsJPEG / DefineBitsLossless tags) as fallback art candidates.
- Pull a title guess from the filename and any metadata tag, then let the user confirm or edit it in an import dialog before the scraper runs.

### 3. Metadata and art scraping
Build a pluggable scraper system: each source is a module behind one shared interface (search, fetch details, fetch art), so I can add sources later.

Sources, in order of priority:
1. **Flashpoint Archive** database/API for Flash-specific metadata (title, developer, publisher, release date, description, tags, logos, screenshots). Check the current Flashpoint docs for the right endpoints before writing code, because they change.
2. **SteamGridDB** API for Steam-style art: grids (covers, 600x900 and 920x430), heroes (1920x620), logos (transparent PNG), and icons. Needs a user API key entered in settings.
3. **IGDB** (Twitch OAuth) for the bigger commercial Flash titles that have entries there.
4. **Internet Archive** search as a last resort.

Scraper behavior:
- Fuzzy match titles (normalize punctuation, strip "v1.2", "_final", site prefixes and similar junk) and show the top matches with thumbnails so the user picks the right one. Auto-accept only when confidence is very high.
- For every art slot (cover, wide cover, hero, logo, icon, screenshots) let the user flip between the candidates from every source, or upload their own image.
- Cache all downloaded images locally, keep them in the right sizes, and never re-download what's already cached.
- Rate limit per source, retry with backoff, and fail quietly: a missing source should never break the import.
- If nothing is found, generate fallback art: a flat Metro tile in an accent color with the title in big type, plus the best embedded image from the SWF if one exists.
- A "rescrape" action per game and a "scrape everything missing art" bulk action with a progress bar.

### 4. Player
- Play games in an in-app player view using Ruffle, either fullscreen or windowed inside the shell.
- Player overlay (opens with a key like Esc or F1): pause, mute, volume, fullscreen, quality, scale mode (fit / stretch / original), take a screenshot (saved to that game's gallery), and quit back to the library.
- Per-game settings saved in the DB: scale mode, quality, frame rate override, letterbox color, and runtime (Ruffle or projector).
- Keep each game's Flash local storage (SharedObjects) so saves persist between sessions. Back them up when the user deletes a game.
- Gamepad support for both the library UI and mapping a controller to keyboard keys in-game (per-game mapping).

## Design: Frutiger Metro / Windows 8
This part matters as much as the features. Get it right.

- **Typography first.** Use Selawik (Microsoft's open-source Segoe UI fallback) or Segoe UI when it's available. Huge, thin, lowercase section headers ("library", "recent", "collections") that run off the edge of the screen, like Windows Phone pivots and hubs. Clear contrast between header weight (Light) and body weight (Regular/Semibold).
- **Flat and sharp.** Zero border radius anywhere. No drop shadows, no gradients, no glass or blur. Solid fills only.
- **Live tiles.** The home screen is a panoramic, horizontally scrolling hub of tiles in mixed sizes (small 1x1, medium 2x2, wide 4x2, large 4x4). Wide and large tiles for recent/featured games flip or slide between cover art, the hero image, and stats (play time, last played) every few seconds, staggered so they don't all move at once.
- **Accent color system.** One user-chosen accent color (offer the classic Win8 palette: cobalt, emerald, crimson, magenta, amber, teal, and so on) used for tiles without art, focus states, progress bars and highlights. Dark theme by default (near-black #111 or so), with a light theme option.
- **Motion.** Use Metro-style animations: turnstile page transitions, tiles tilting slightly toward the click point on press, continuum transitions (the game's tile expands into its detail page), staggered slide-in of list items. Animations should be quick, around 150 to 350ms with strong ease-out. Respect prefers-reduced-motion.
- **Layout.** Lots of negative space, content left-aligned to a strict grid, horizontal scrolling for hubs, vertical for the detail page. Use the hero image on the game page full-bleed with the logo laid over it, and a dark scrim only where the text needs it.
- **Chrome.** A custom frameless title bar (Tauri decorations off) with minimal window controls. A slim left nav or top pivot: home, library, collections, add game, settings.
- Make it fully usable with keyboard and gamepad: visible accent-color focus rings, arrow-key navigation through tile grids.

## Settings
- API keys (SteamGridDB, IGDB client ID/secret).
- Library folder location, plus a "scan this folder for SWFs" option that watches for new files.
- Accent color and theme.
- Default player settings.
- Projector path (optional).
- Export/import the library (DB plus art plus saves) as one zip for backups.

## Engineering notes
- Keep the Ruffle runtime, scrapers, SWF parser, and UI as separate modules with clean boundaries.
- Put file I/O, hashing, SWF parsing, and network fetching in Rust commands. The UI shouldn't do heavy work.
- Run SWFs only from the app's own data folder, sandboxed: block games from reaching the network by default, with a per-game toggle for the few that need it (some archived games call home for sitelocks or high scores).
- Write a README covering setup, getting API keys, and how to add a new scraper source.
- Add tests for the SWF header parser and the title-normalization/matching code.

## Build order
1. App shell, design system (fonts, colors, tile components, animations), and the navigation.
2. SWF import, header parsing, and library DB.
3. Ruffle player view and overlay.
4. Scraper framework with Flashpoint and SteamGridDB, then IGDB and Internet Archive.
5. Home hub with live tiles, game detail page, collections.
6. Gamepad support, settings, backup/export.

Stop after each step, show me screenshots or tell me how to run it, and wait for my OK before moving on. Ask me before picking any dependency not listed here.
