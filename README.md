# Flash Launcher

A small, fast desktop launcher for Flash games and animations from the [Flashpoint Archive](https://flashpointarchive.org/), plus a plain `.swf` player for your own files. Games run in [Ruffle](https://ruffle.rs/), so there's no Adobe Flash anywhere.

![Home](docs/screenshots/home.png)

## What it does

- **Browse Flashpoint's catalog.** Instant search across tens of thousands of games and animations, with filters for genre tags, games vs animations, and "verified with Ruffle".
- **Download only what you play.** Each game is a single GameZIP pulled from Flashpoint on demand. Downloads resume if interrupted and are checked against Flashpoint's SHA-256 before they count.
- **Play offline.** A built-in local proxy acts as a fake copy of the original website, so games that check their URL (sitelocks) or load files from their own domain still work.
- **Open your own SWFs.** Drag and drop, use the file picker, or double-click a `.swf` (file association is set up by the installer). An optional "pretend URL" makes sitelocked files think they're on their original site.
- **Three ways to play.** The in-app Ruffle player (default), Ruffle desktop for heavier AS3 games (downloaded once, on demand), or your own Flash Player projector as a last resort.
- **Liquid glass UI.** Translucent, blurred panels over the game's own art, with smooth animations. Mica/acrylic on Windows 11. Reduce-motion and glass intensity are in Settings.
- **Optional SteamGridDB art.** Add your own free API key to get covers and hero images where SteamGridDB has them.

## Build and run (Windows)

You need **Rust** (stable), **Node.js 22+**, and the **WebView2 runtime** (already on Windows 10/11).

```powershell
npm install          # also copies Ruffle's web build into src-tauri/resources/ruffle
npm run tauri dev    # run with hot reload
npm run tauri build  # installers in target/release/bundle/{nsis,msi}
```

macOS 14+ and Linux (WebKitGTK 4.1) build the same way.

### First run: the catalog

The app downloads a prebuilt catalog from this repo's `catalog` release. That release is produced by the **Build catalog** GitHub Action (`.github/workflows/catalog.yml`), which runs weekly. After pushing the repo to `github.com/caidn/FlashLauncher`, run that workflow once from the Actions tab.

Until then, the welcome screen has **"Or sync directly from Flashpoint (slow)"**, which builds the catalog on your machine straight from Flashpoint's API. It works, but it pulls a few hundred MB of JSON, so it's a fallback.

## How it works

```
Main window (Svelte UI) ── Tauri commands ──▶ fl-core (Rust)
                                               ├─ catalog: SQLite + FTS5
                                               ├─ downloads: resumable, sha256-verified
Player window ── proxy_url ──▶ local proxy ────┤
  loads http://<game's original host>/__fl/embed.html
                                               └─ serves: bundled Ruffle → mounted GameZIP
                                                  → htdocs cache → Flashpoint remote htdocs
                                                  → 404 (never the real internet)
```

The player is its own webview whose proxy is set to the local server, so the game page really is at its original domain. It's the same trick Flashpoint's own launcher uses with Chrome. Details: [`docs/architecture.md`](docs/architecture.md) and [`docs/flashpoint-schema.md`](docs/flashpoint-schema.md).

`crates/fl-core` has everything except UI and has no Tauri dependency, so it's unit tested on its own (`cargo test -p fl-core`).

## Tools

| Command | What it's for |
|---|---|
| `cargo run -p fl-core --example probe` | Checks every Flashpoint endpoint from your machine and writes `docs/probe/report.md`. Run this first, and whenever something breaks. |
| `cargo run -p fl-core --example serve -- game.zip <launch url>` | Runs the proxy alone against a GameZIP, for testing in a normal browser with `--proxy-server`. |
| `python scripts/make-test-swf.py test.zip` | Builds a tiny test GameZIP (two squares; the second loads by relative URL). |
| `cargo run -p fl-core --bin fl-catalog -- build --out dist` | Builds the catalog the same way CI does. |

## Being a good citizen

Flashpoint runs on donations. Flash Launcher never prefetches, limits itself to 2 GameZIP downloads and 6 image fetches at once, backs off on 429/5xx, caches everything, and identifies itself with a `User-Agent` pointing back here. The catalog is built once a week in CI instead of by every user.

## Known limitations

- HTTPS assets aren't served (the proxy refuses `CONNECT`). Almost all Flash-era content is plain HTTP.
- Games that need server-side scripts (PHP high-score servers, multiplayer) won't get those parts.
- Legacy (non-GameZIP) Flashpoint entries, Shockwave, Unity and Java aren't supported.
- The Flash projector option can't use the proxy (projectors only use the system proxy, which this app never touches), so sitelocked games may not run that way.
- Screenshots of the in-app player depend on the renderer and can come out blank.

## Credits

All game metadata, files and artwork come from **Flashpoint Archive**. Please [donate](https://opencollective.com/flashpointarchive) if you enjoy this.
Emulation by **Ruffle** ([sponsor](https://github.com/sponsors/ruffle-rs)). Not affiliated with either project or with Adobe.

MIT licensed. Game content belongs to its creators.
