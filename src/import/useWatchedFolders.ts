import { useCallback, useEffect, useRef } from "react";
import { useScrape } from "../art/ScrapeProvider";
import { useToast } from "../components/Feedback/Feedback";
import { useLibrary } from "../data/LibraryProvider";
import { api } from "../lib/api";
import { isTauri } from "../lib/tauri";

const MIN_GAP_MS = 60_000;

/** Scans watched folders at startup and whenever the window comes back
 * into focus (at most once a minute), and finds art for what's new. */
export function useWatchedFolders(paused: boolean): () => Promise<number> {
  const { refresh } = useLibrary();
  const { scrapeGames } = useScrape();
  const toast = useToast();
  const last = useRef(0);
  const running = useRef(false);

  const scan = useCallback(async () => {
    if (running.current) return 0;
    running.current = true;
    last.current = Date.now();
    try {
      const r = await api.scanWatched();
      if (r.added.length) {
        await refresh();
        scrapeGames(r.added.map((g) => g.id));
        toast({
          title: r.added.length === 1 ? `added ${r.added[0].title}` : `added ${r.added.length} games`,
          body: "New from your watched folders.",
        });
      }
      return r.added.length;
    } catch {
      return 0;
    } finally {
      running.current = false;
    }
  }, [refresh, scrapeGames, toast]);

  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    if (!isTauri) return;
    scan();
    const onFocus = () => {
      if (!pausedRef.current && Date.now() - last.current > MIN_GAP_MS) scan();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [scan]);

  return scan;
}
