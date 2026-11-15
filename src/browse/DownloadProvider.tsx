import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ProgressBar, useToast } from "../components/Feedback/Feedback";
import { useLibrary } from "../data/LibraryProvider";
import type { BrowseGame, DownloadProgress } from "../data/types";
import { api } from "../lib/api";
import { formatBytes } from "../lib/format";
import s from "../art/ScrapeProvider.module.css";
import d from "./Download.module.css";

interface Job {
  id: string;
  title: string;
}

interface DownloadApi {
  /** Queues a Flashpoint game for download. */
  download: (g: BrowseGame) => void;
  cancel: () => void;
  /** The game being downloaded right now, with its progress. */
  current: (Job & { progress: DownloadProgress | null }) | null;
  queued: string[];
  /** Flashpoint id -> library game id, for games downloaded this session. */
  done: Record<string, number>;
  failed: Record<string, string>;
}

const Ctx = createContext<DownloadApi>({
  download: () => {},
  cancel: () => {},
  current: null,
  queued: [],
  done: {},
  failed: {},
});

export const PHASE_LABEL: Record<DownloadProgress["phase"], string> = {
  download: "downloading",
  unpack: "unpacking",
  import: "adding to your library",
  art: "getting art & info",
  done: "done",
};

export function progressText(p: DownloadProgress | null): string {
  if (!p) return "starting";
  if (p.phase === "download" && p.total > 0) return `${formatBytes(p.received)} of ${formatBytes(p.total)}`;
  if (p.phase === "download" && p.received > 0) return formatBytes(p.received);
  return PHASE_LABEL[p.phase];
}

// Downloads run one at a time in the background with an accent strip
// along the bottom (same look as the art strip), so you can keep browsing.
export function DownloadProvider({ children }: { children: ReactNode }) {
  const { refresh } = useLibrary();
  const toast = useToast();
  const [current, setCurrent] = useState<DownloadApi["current"]>(null);
  const [queued, setQueued] = useState<Job[]>([]);
  const [done, setDone] = useState<Record<string, number>>({});
  const [failed, setFailed] = useState<Record<string, string>>({});
  const queue = useRef<Job[]>([]);
  const busy = useRef(false);

  useEffect(() => {
    document.documentElement.dataset.downloading = String(!!current);
  }, [current]);

  useEffect(() => {
    let off = () => {};
    api
      .onDownloadProgress((p) => setCurrent((c) => (c && c.id === p.id ? { ...c, progress: p } : c)))
      .then((fn) => (off = fn));
    return () => off();
  }, []);

  const pump = useCallback(async () => {
    if (busy.current) return;
    const job = queue.current.shift();
    setQueued([...queue.current]);
    if (!job) return;
    busy.current = true;
    setCurrent({ ...job, progress: null });
    try {
      const r = await api.downloadGame(job.id);
      await refresh();
      setDone((prev) => ({ ...prev, [job.id]: r.game.id }));
      toast({
        title: r.already ? "already in your library" : `${r.game.title} is ready`,
        body: r.already ? `${r.game.title} was there already.` : "It's in your library now.",
      });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      setFailed((prev) => ({ ...prev, [job.id]: msg }));
      if (!/cancelled/.test(msg)) toast({ title: `couldn't download ${job.title}`, body: msg });
    } finally {
      busy.current = false;
      setCurrent(null);
      pump();
    }
  }, [refresh, toast]);

  const download = useCallback(
    (g: BrowseGame) => {
      if (queue.current.some((j) => j.id === g.id)) return;
      setFailed((prev) => {
        const next = { ...prev };
        delete next[g.id];
        return next;
      });
      queue.current.push({ id: g.id, title: g.title });
      setQueued([...queue.current]);
      pump();
    },
    [pump],
  );

  const cancel = useCallback(() => {
    api.cancelDownload().catch(() => {});
  }, []);

  return (
    <Ctx.Provider value={{ download, cancel, current, queued: queued.map((j) => j.id), done, failed }}>
      {children}
      {current &&
        createPortal(
          <div className={`${s.strip} ${d.strip}`} role="status">
            <div className={s.text}>
              <span className={s.label}>{current.title}</span>
              <span className={s.current}>
                {progressText(current.progress)}
                {queued.length > 0 && ` · ${queued.length} more queued`}
              </span>
            </div>
            <div className={s.bar}>
              {current.progress?.phase === "download" && current.progress.total > 0 ? (
                <ProgressBar value={current.progress.received} max={current.progress.total} />
              ) : (
                <ProgressBar value={current.progress?.phase === "done" ? 1 : 0} max={1} />
              )}
            </div>
            <button className={s.stop} onClick={cancel}>
              cancel
            </button>
          </div>,
          document.getElementById("layer") ?? document.body,
        )}
    </Ctx.Provider>
  );
}

export function useDownloads(): DownloadApi {
  return useContext(Ctx);
}
