import { useEffect, useState } from "react";
import { Button, Segmented, TextField, Toggle } from "../../components/Controls/Controls";
import { ConfirmDialog } from "../../components/Dialog/Dialogs";
import { ProgressBar, useToast } from "../../components/Feedback/Feedback";
import { Icon } from "../../components/Icon/Icon";
import type { BackupProgress, Quality, ScaleMode } from "../../data/types";
import { useScrape } from "../../art/ScrapeProvider";
import { useLibrary } from "../../data/LibraryProvider";
import { api } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { useUpdates } from "../../update/UpdateProvider";
import notices from "../../../THIRD_PARTY_NOTICES.md?raw";
import { BandDialog } from "../../components/Dialog/BandDialog";
import { ACCENTS, accentById, readableOn } from "../../theme/accents";
import { useTheme } from "../../theme/ThemeProvider";
import p from "../page.module.css";
import s from "./Settings.module.css";

export function Settings() {
  const { accent, setAccent, mode, setMode, motion, setMotion } = useTheme();
  const { games, refresh } = useLibrary();
  const toast = useToast();
  const [dataDir, setDataDir] = useState("");
  const [projector, setProjector] = useState("");
  const [keys, setKeys] = useState({ steamgriddbKey: "", igdbClientId: "", igdbClientSecret: "" });
  const [defaults, setDefaults] = useState<{ scale: ScaleMode; quality: Quality; network: boolean }>({
    scale: "fit",
    quality: "high",
    network: false,
  });
  const [folders, setFolders] = useState<string[]>([]);
  const [discord, setDiscord] = useState(true);
  const [showNotices, setShowNotices] = useState(false);
  const [updateNote, setUpdateNote] = useState<string | null>(null);
  const updates = useUpdates();
  const [scanning, setScanning] = useState(false);
  const [backup, setBackup] = useState<BackupProgress | null>(null);
  const [restoreFrom, setRestoreFrom] = useState<string | null>(null);
  const { scrapeGames, running: scraping } = useScrape();
  useEffect(() => {
    api.dataDir().then(setDataDir).catch(() => {});
    api
      .getSettings()
      .then((s) => {
        setProjector(s.projectorPath ?? "");
        setKeys({
          steamgriddbKey: s.steamgriddbKey ?? "",
          igdbClientId: s.igdbClientId ?? "",
          igdbClientSecret: s.igdbClientSecret ?? "",
        });
        setDefaults({
          scale: (s.defaultScaleMode as ScaleMode) ?? "fit",
          quality: (s.defaultQuality as Quality) ?? "high",
          network: s.defaultAllowNetwork === "true",
        });
        setDiscord(s.discordPresence !== "false");
        try {
          setFolders(JSON.parse(s.watchedFolders ?? "[]"));
        } catch {
          setFolders([]);
        }
      })
      .catch(() => {});
    let off = () => {};
    api.onBackupProgress(setBackup).then((fn) => (off = fn));
    return () => off();
  }, []);

  const setDefault = <K extends keyof typeof defaults>(k: K, v: (typeof defaults)[K]) => {
    setDefaults((d) => ({ ...d, [k]: v }));
    const key = { scale: "defaultScaleMode", quality: "defaultQuality", network: "defaultAllowNetwork" }[k];
    api.setSetting(key, String(v)).catch(() => {});
  };

  const saveFolders = (next: string[]) => {
    setFolders(next);
    api.setSetting("watchedFolders", JSON.stringify(next)).catch(() => {});
  };

  const scanNow = async () => {
    setScanning(true);
    try {
      const r = await api.scanWatched();
      if (r.added.length) {
        await refresh();
        scrapeGames(r.added.map((g) => g.id));
      }
      toast({
        title: r.added.length ? `added ${r.added.length} ${r.added.length === 1 ? "game" : "games"}` : "nothing new",
        body: r.failed.length ? `${r.failed.length} files couldn't be read.` : `Checked ${r.scanned} SWF files.`,
      });
    } catch (e) {
      toast({ title: "scan failed", body: String(e) });
    } finally {
      setScanning(false);
    }
  };

  const backUp = async () => {
    const day = new Date().toISOString().slice(0, 10);
    const path = await api.pickBackupTarget(`flash-launcher-backup-${day}.zip`);
    if (!path) return;
    setBackup({ phase: "pack", done: 0, total: 1 });
    try {
      const size = await api.exportLibrary(path);
      toast({ title: "backup saved", body: `${formatBytes(size)} · ${path}` });
    } catch (e) {
      toast({ title: "backup failed", body: String(e) });
    } finally {
      setBackup(null);
    }
  };

  const restore = async (path: string) => {
    setRestoreFrom(null);
    setBackup({ phase: "unpack", done: 0, total: 1 });
    try {
      await api.restoreLibrary(path);
      api.reloadApp();
    } catch (e) {
      toast({ title: "restore failed", body: String(e) });
      setBackup(null);
    }
  };
  const saveKey = (k: keyof typeof keys, v: string) => {
    setKeys((prev) => ({ ...prev, [k]: v }));
    api.setSetting(k, v.trim()).catch(() => {});
  };
  const saveProjector = (v: string) => {
    setProjector(v);
    api.setSetting("projectorPath", v.trim()).catch(() => {});
  };
  const totalBytes = games.reduce((n, g) => n + g.fileSize, 0);
  const downloaded = games.filter((g) => g.contentId).length;

  return (
    <div className={p.page}>
      <h1 className={`${p.title} ts`} style={{ ["--i" as string]: 0 }}>
        settings
      </h1>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 1 }}>
        <h2 className={p.sectionTitle}>accent color</h2>
        <p className={s.current}>{accentById(accent).name}</p>
        <div className={s.swatches} role="radiogroup" aria-label="Accent color">
          {ACCENTS.map((a) => (
            <button
              key={a.id}
              role="radio"
              aria-checked={accent === a.id}
              aria-label={a.name}
              title={a.name}
              className={s.swatch}
              data-active={accent === a.id}
              style={{ background: a.hex, color: readableOn(a.hex) }}
              onClick={() => setAccent(a.id)}
              data-nav
            />
          ))}
        </div>
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 2 }}>
        <h2 className={p.sectionTitle}>appearance</h2>
        <Toggle
          label="dark theme"
          hint="Light theme is white backgrounds with dark text."
          checked={mode === "dark"}
          onChange={(v) => setMode(v ? "dark" : "light")}
        />
        <Toggle
          label="reduce motion"
          hint="Turns off tile flips and page animations. Your system setting is always respected too."
          checked={motion === "reduced"}
          onChange={(v) => setMotion(v ? "reduced" : "system")}
        />
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 3 }}>
        <h2 className={p.sectionTitle}>player</h2>
        <p className={`${p.body} ${s.blurb}`}>
          Games run in Ruffle. For AS3 games Ruffle can't handle yet, point this at your own copy of Adobe's standalone
          Flash Player (the "projector"), then switch the game to it from its in-game menu.
        </p>
        <div className={s.pathRow}>
          <TextField label="Flash Player projector path" value={projector} onChange={saveProjector} />
          <Button
            onClick={async () => {
              const exe = await api.pickExe();
              if (exe) saveProjector(exe);
            }}
          >
            browse
          </Button>
        </div>
        <p className={s.keys}>
          In a game: <kbd>esc</kbd> or <kbd>F1</kbd> menu · <kbd>F11</kbd> full screen · <kbd>F12</kbd> screenshot ·
          controller: <kbd>view</kbd> + <kbd>menu</kbd> menu
        </p>

        <div className={s.gap}>
          <Toggle
            label="show what I'm playing on Discord"
            hint="Your Discord profile shows the game, its art and how long you've been playing. Needs the Discord app running on this computer."
            checked={discord}
            onChange={(v) => {
              setDiscord(v);
              api.setSetting("discordPresence", String(v)).catch(() => {});
              if (!v) api.discordClear().catch(() => {});
            }}
          />
        </div>

        <h3 className={s.subTitle}>new games start with</h3>
        <div className={s.defaults}>
          <span>scaling</span>
          <Segmented<ScaleMode>
            label="Default scaling"
            value={defaults.scale}
            onChange={(v) => setDefault("scale", v)}
            options={[
              { value: "fit", label: "fit" },
              { value: "stretch", label: "stretch" },
              { value: "original", label: "original size" },
            ]}
          />
          <span>quality</span>
          <Segmented<Quality>
            label="Default quality"
            value={defaults.quality}
            onChange={(v) => setDefault("quality", v)}
            options={[
              { value: "low", label: "low" },
              { value: "medium", label: "medium" },
              { value: "high", label: "high" },
              { value: "best", label: "best" },
            ]}
          />
        </div>
        <Toggle
          label="let games use the internet"
          hint="Off keeps games offline: no ads, no tracking, no dead servers to wait on. Some games need it for high scores or extra levels."
          checked={defaults.network}
          onChange={(v) => setDefault("network", v)}
        />
        <div className={s.actionsRow}>
          <Button
            onClick={async () => {
              await api.applyPlayerDefaults();
              await refresh();
              toast({ title: "done", body: "Every game uses these settings now." });
            }}
          >
            use these for every game
          </Button>
          <span className={s.actionsHint}>Each game can still be changed from its in-game menu.</span>
        </div>
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 4 }}>
        <h2 className={p.sectionTitle}>watched folders</h2>
        <p className={`${p.body} ${s.blurb}`}>
          SWFs you save into these folders show up in your library on their own, checked when the launcher starts and
          when you come back to it. Games you delete from the library don't come back.
        </p>
        {folders.length > 0 && (
          <ul className={s.folders}>
            {folders.map((f) => (
              <li key={f}>
                <Icon name="folder" size={16} />
                <span className={s.path}>{f}</span>
                <button className={s.remove} onClick={() => saveFolders(folders.filter((x) => x !== f))} data-nav>
                  remove
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className={s.actionsRow}>
          <Button
            onClick={async () => {
              const f = await api.pickFolder();
              if (f && !folders.includes(f)) saveFolders([...folders, f]);
            }}
          >
            <Icon name="plus" size={14} /> add a folder
          </Button>
          <Button onClick={scanNow} disabled={scanning || folders.length === 0}>
            {scanning ? "checking…" : "check now"}
          </Button>
        </div>
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 5 }}>
        <h2 className={p.sectionTitle}>art & info</h2>
        <p className={`${p.body} ${s.blurb}`}>
          Flashpoint Archive and the Internet Archive work out of the box. For Steam-style covers, banners and logos,
          add a free SteamGridDB key. IGDB adds details and covers for bigger commercial games. Keys stay on this
          computer.
        </p>
        <div className={s.keyGrid}>
          <label>
            <span>SteamGridDB API key</span>
            <TextField label="SteamGridDB API key" value={keys.steamgriddbKey} onChange={(v) => saveKey("steamgriddbKey", v)} />
            <small>steamgriddb.com → profile → preferences → API</small>
          </label>
          <label>
            <span>IGDB client ID</span>
            <TextField label="IGDB client ID" value={keys.igdbClientId} onChange={(v) => saveKey("igdbClientId", v)} />
            <small>dev.twitch.tv/console/apps → register an app</small>
          </label>
          <label>
            <span>IGDB client secret</span>
            <TextField label="IGDB client secret" value={keys.igdbClientSecret} onChange={(v) => saveKey("igdbClientSecret", v)} />
            <small>from the same Twitch app</small>
          </label>
        </div>
        <div className={s.actionsRow}>
          <Button variant="accent" onClick={() => scrapeGames(null)} disabled={scraping}>
            {scraping ? "finding art…" : "find missing art for all games"}
          </Button>
          <span className={s.actionsHint}>Games you've already matched or picked art for are skipped.</span>
        </div>
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 6 }}>
        <h2 className={p.sectionTitle}>library</h2>
        <dl className={s.facts}>
          <dt>games</dt>
          <dd>
            {games.length} · {formatBytes(totalBytes)}
            {downloaded > 0 && ` · ${downloaded} downloaded from Flashpoint`}
          </dd>
          <dt>stored in</dt>
          <dd className={s.path}>
            {dataDir}{" "}
            {dataDir && (
              <button className={s.inlineLink} onClick={() => api.openExternal(dataDir).catch(() => {})}>
                open
              </button>
            )}
          </dd>
        </dl>
        <p className={`${p.body} ${s.blurb} ${s.gap}`}>
          A backup is one zip with everything: games, saves, play time, collections, art, downloads and settings
          (API keys included, so keep it somewhere private). Restoring replaces your current library; the old one is
          moved to a before-restore folder next to it, just in case.
        </p>
        <div className={s.actionsRow}>
          <Button variant="accent" onClick={backUp} disabled={!!backup}>
            back up library
          </Button>
          <Button
            onClick={async () => {
              const f = await api.pickBackup();
              if (f) setRestoreFrom(f);
            }}
            disabled={!!backup}
          >
            restore from a backup
          </Button>
          {backup && (
            <span className={s.backupProgress}>
              <span>{backup.phase === "pack" ? "packing" : "unpacking"}</span>
              <ProgressBar value={backup.done} max={backup.total || 1} />
            </span>
          )}
        </div>
      </section>

      <section className={`${p.section} ts`} style={{ ["--i" as string]: 7 }}>
        <h2 className={p.sectionTitle}>about</h2>
        <p className={p.body}>
          Flash Launcher {updates.version || "(preview)"}. Flash runs through Ruffle. Game info, art and downloads come
          from Flashpoint Archive, with more art and details from SteamGridDB, IGDB and the Internet Archive.
        </p>
        <div className={s.actionsRow}>
          {updates.available && (
            <Button
              onClick={async () => {
                setUpdateNote(null);
                try {
                  const got = await updates.check();
                  setUpdateNote(got ? "There's an update: see the bar along the bottom." : "You're on the newest version.");
                } catch (e) {
                  setUpdateNote(`Couldn't check: ${e instanceof Error ? e.message : String(e)}`);
                }
              }}
              disabled={updates.checking}
            >
              {updates.checking ? "checking…" : "check for updates"}
            </Button>
          )}
          <Button onClick={() => setShowNotices(true)}>open-source licenses</Button>
          {updateNote && <span className={s.actionsHint}>{updateNote}</span>}
        </div>
      </section>

      {showNotices && (
        <BandDialog
          title="open-source licenses"
          onCancel={() => setShowNotices(false)}
          wide
          tall
          actions={
            <Button variant="accent" onClick={() => setShowNotices(false)} data-autofocus>
              close
            </Button>
          }
        >
          <pre className={s.notices}>{notices}</pre>
        </BandDialog>
      )}

      {restoreFrom && (
        <ConfirmDialog
          title="replace your library?"
          confirmLabel="restore"
          onClose={() => setRestoreFrom(null)}
          onConfirm={() => restore(restoreFrom)}
        >
          <p>
            Everything in your library now gets swapped for what's in {restoreFrom.split(/[\\/]/).pop()}. Your current
            library is kept in a before-restore folder in case you change your mind. The launcher reloads when it's done.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}
