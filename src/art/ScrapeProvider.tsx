import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { ProgressBar, useToast } from "../components/Feedback/Feedback";
import { useLibrary } from "../data/LibraryProvider";
import type { ScrapeProgress } from "../data/types";
import { api } from "../lib/api";
import s from "./ScrapeProvider.module.css";

interface ScrapeApi {
  /** Finds info and art for these games, or for every game missing art. */
  scrapeGames: (ids: number[] | null) => void;
  running: boolean;
}

const Ctx = createContext<ScrapeApi>({ scrapeGames: () => {}, running: false });

// Runs bulk scrapes in the background with a slim progress strip along
// the bottom, so the library stays usable while art comes in.
export function ScrapeProvider({ children }: { children: ReactNode }) {
  const { refresh } = useLibrary();
  const toast = useToast();
  const [progress, setProgress] = useState<ScrapeProgress | null>(null);
  const [running, setRunning] = useState(false);
  const queue = useRef<(number[] | null)[]>([]);
  const busy = useRef(false);

  // Lets the bottom app bar sit above the progress strip.
  useEffect(() => {
    document.documentElement.dataset.scraping = String(running);
  }, [running]);

  useEffect(() => {
    let off = () => {};
    api.onScrapeProgress(setProgress).then((fn) => (off = fn));
    return () => off();
  }, []);

  const pump = useCallback(async () => {
    if (busy.current) return;
    const next = queue.current.shift();
    if (next === undefined) return;
    busy.current = true;
    setRunning(true);
    setProgress(null);
    try {
      const r = await api.scrapeBulk(next);
      await refresh();
      const matched = r.reports.filter((x) => x.linked.length > 0).length;
      const review = r.reports.filter((x) => x.needsReview).length;
      const offline = r.reports.some((x) => x.errors.some((e) => /reach|timed out|online/i.test(e)));
      if (r.reports.length) {
        toast({
          title: r.cancelled ? "stopped finding art" : matched ? `found info for ${matched} ${matched === 1 ? "game" : "games"}` : "no matches found",
          body: offline
            ? "Some sources couldn't be reached. Check your connection."
            : review
              ? `${review} need a hand: right-click them in the library and choose "find info".`
              : "Art and details are in your library.",
        });
      }
    } catch (e) {
      toast({ title: "finding art failed", body: String(e) });
    } finally {
      busy.current = false;
      setRunning(false);
      setProgress(null);
      pump();
    }
  }, [refresh, toast]);

  const scrapeGames = useCallback(
    (ids: number[] | null) => {
      if (ids && !ids.length) return;
      queue.current.push(ids);
      pump();
    },
    [pump],
  );

  return (
    <Ctx.Provider value={{ scrapeGames, running }}>
      {children}
      {running &&
        createPortal(
          <div className={s.strip} role="status">
            <div className={s.text}>
              <span className={s.label}>finding art & info</span>
              <span className={s.current}>
                {progress && progress.total > 0
                  ? `${Math.min(progress.done + 1, progress.total)} of ${progress.total}${progress.current ? ` · ${progress.current}` : ""}`
                  : "starting"}
              </span>
            </div>
            <div className={s.bar}>
              <ProgressBar value={progress?.done ?? 0} max={progress?.total || 1} />
            </div>
            <button className={s.stop} onClick={() => api.scrapeCancel()}>
              stop
            </button>
          </div>,
          document.getElementById("layer") ?? document.body,
        )}
    </Ctx.Provider>
  );
}

export function useScrape(): ScrapeApi {
  return useContext(Ctx);
}
