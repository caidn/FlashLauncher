import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ProgressBar, useToast } from "../components/Feedback/Feedback";
import { formatBytes } from "../lib/format";
import { isTauri } from "../lib/tauri";
import s from "../art/ScrapeProvider.module.css";
import u from "./Update.module.css";

// In-app updates from GitHub Releases. Checks once a day at startup; a
// found update shows as an accent strip along the bottom with "update now".
// Installing downloads the signed installer, runs it, and restarts.

type Found = { version: string; notes: string | null; install: (cb: (done: number, total: number) => void) => Promise<void> };

interface UpdateApi {
  /** The app's own version (from the build). */
  version: string;
  /** Updates need the signing key set up; false in dev builds without it. */
  available: boolean;
  found: Found | null;
  checking: boolean;
  /** Looks for an update now; resolves to whether one was found. */
  check: () => Promise<boolean>;
  install: () => void;
}

const Ctx = createContext<UpdateApi>({
  version: "",
  available: false,
  found: null,
  checking: false,
  check: async () => false,
  install: () => {},
});

const LAST_CHECK = "flashlauncher.lastUpdateCheck";
const DISMISSED = "flashlauncher.dismissedUpdate";
const DAY = 24 * 60 * 60 * 1000;

function store(key: string, value?: string): string | null {
  try {
    if (value !== undefined) localStorage.setItem(key, value);
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

export function UpdateProvider({ children, paused }: { children: ReactNode; paused: boolean }) {
  const toast = useToast();
  const [version, setVersion] = useState("");
  const [available, setAvailable] = useState(false);
  const [found, setFound] = useState<Found | null>(null);
  const [checking, setChecking] = useState(false);
  const [progress, setProgress] = useState<{ done: number; total: number } | null>(null);
  const [hidden, setHidden] = useState(false);

  const check = useCallback(async () => {
    if (!isTauri) return false;
    setChecking(true);
    try {
      const { check: checkUpdate } = await import("@tauri-apps/plugin-updater");
      store(LAST_CHECK, String(Date.now()));
      const update = await checkUpdate();
      if (!update) {
        setFound(null);
        return false;
      }
      setFound({
        version: update.version,
        notes: update.body ?? null,
        install: async (cb) => {
          let total = 0;
          let done = 0;
          await update.downloadAndInstall((e) => {
            if (e.event === "Started") total = e.data.contentLength ?? 0;
            if (e.event === "Progress") done += e.data.chunkLength;
            cb(done, total);
          });
        },
      });
      setHidden(store(DISMISSED) === update.version);
      return true;
    } finally {
      setChecking(false);
    }
  }, []);

  useEffect(() => {
    if (!isTauri) return;
    (async () => {
      const [{ getVersion }, { invoke }] = await Promise.all([
        import("@tauri-apps/api/app"),
        import("@tauri-apps/api/core"),
      ]);
      setVersion(await getVersion());
      const ok = await invoke<boolean>("updates_configured").catch(() => false);
      setAvailable(ok);
      const last = Number(store(LAST_CHECK) ?? 0);
      if (ok && Date.now() - last > DAY) check().catch(() => {});
    })();
  }, [check]);

  const install = useCallback(async () => {
    if (!found) return;
    setProgress({ done: 0, total: 0 });
    try {
      await found.install((done, total) => setProgress({ done, total }));
      // Windows closes the app to run the installer; elsewhere, restart.
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch (e) {
      setProgress(null);
      toast({ title: "update failed", body: e instanceof Error ? e.message : String(e) });
    }
  }, [found, toast]);

  const dismiss = () => {
    if (found) store(DISMISSED, found.version);
    setHidden(true);
  };

  const show = !!found && (!hidden || !!progress) && !paused;
  useEffect(() => {
    document.documentElement.dataset.updating = String(show);
  }, [show]);

  return (
    <Ctx.Provider value={{ version, available, found, checking, check, install }}>
      {children}
      {show &&
        createPortal(
          <div className={`${s.strip} ${u.strip}`} role="status">
            <div className={s.text}>
              <span className={s.label}>Flash Launcher {found.version} is out</span>
              <span className={s.current}>
                {progress
                  ? progress.total
                    ? `downloading · ${formatBytes(progress.done)} of ${formatBytes(progress.total)}`
                    : "downloading"
                  : `you have ${version}. Updating restarts the launcher.`}
              </span>
            </div>
            {progress ? (
              <div className={s.bar}>
                <ProgressBar value={progress.done} max={progress.total || 1} />
              </div>
            ) : (
              <>
                <button className={s.stop} onClick={install}>
                  update now
                </button>
                <button className={u.later} onClick={dismiss}>
                  later
                </button>
              </>
            )}
          </div>,
          document.getElementById("layer") ?? document.body,
        )}
    </Ctx.Provider>
  );
}

export function useUpdates(): UpdateApi {
  return useContext(Ctx);
}
