import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Pivot } from "./components/Pivot/Pivot";
import { TitleBar } from "./components/TitleBar/TitleBar";
import { useLibrary } from "./data/LibraryProvider";
import { ImportProvider } from "./import/ImportProvider";
import { useWatchedFolders } from "./import/useWatchedFolders";
import { api } from "./lib/api";
import { installGamepad } from "./lib/gamepad";
import { installSpatialNav } from "./lib/spatialNav";
import { isTauri } from "./lib/tauri";
import { NavContext, type Nav } from "./nav";
import { AddGame } from "./pages/AddGame/AddGame";
import { Browse } from "./pages/Browse/Browse";
import { CollectionView } from "./pages/Collections/CollectionView";
import { Collections } from "./pages/Collections/Collections";
import { GamePage, type Origin } from "./pages/Game/GamePage";
import { Home } from "./pages/Home/Home";
import { Library } from "./pages/Library/Library";
import { Settings } from "./pages/Settings/Settings";
import { Player } from "./player/Player";
import type { Route } from "./routes";
import { useReducedMotion } from "./theme/ThemeProvider";
import { UpdateProvider } from "./update/UpdateProvider";
import s from "./App.module.css";

const ROUTES: Route[] = ["home", "library", "collections", "browse", "add", "settings"];
/** The pivot pages LB / RB cycle through. */
const PIVOT: Route[] = ["home", "library", "collections", "browse"];
const OUT_MS = 160;

function routeFromHash(): Route {
  const h = window.location.hash.replace("#/", "").replace("#", "") as Route;
  return ROUTES.includes(h) ? h : "home";
}

/** What fills the main area. Pages are top level; the rest stack on top. */
type View =
  | { kind: "page"; route: Route }
  | { kind: "game"; gameId: number; origin?: Origin }
  | { kind: "collection"; id: number }
  | { kind: "play"; gameId: number };

export function App() {
  const { games, collections } = useLibrary();
  const [stack, setStack] = useState<View[]>(() => [{ kind: "page", route: routeFromHash() }]);
  const [shown, setShown] = useState<View>(stack[0]);
  const [phase, setPhase] = useState<"in" | "out">("in");
  const [visit, setVisit] = useState(0);
  const [fullscreen, setFullscreen] = useState(false);
  const reduced = useReducedMotion();
  const timer = useRef<number>(0);

  useEffect(() => installSpatialNav(), []);

  // Turnstile out, swap, turnstile in. Games opened from a tile skip the
  // out step so the page can grow straight out of that tile.
  const show = useCallback(
    (next: View, skipOut = false) => {
      window.clearTimeout(timer.current);
      const swap = () => {
        setShown(next);
        setPhase("in");
        setVisit((v) => v + 1);
      };
      if (reduced || skipOut) return swap();
      setPhase("out");
      timer.current = window.setTimeout(swap, OUT_MS);
    },
    [reduced],
  );

  const go = useCallback(
    (nextStack: View[], skipOut = false) => {
      setStack(nextStack);
      const top = nextStack[nextStack.length - 1];
      if (top.kind === "page") window.history.replaceState(null, "", `#/${top.route}`);
      show(top, skipOut);
    },
    [show],
  );

  const top = stack[stack.length - 1];
  const route: Route = (stack.find((v) => v.kind === "page") as { route: Route } | undefined)?.route ?? "home";

  const navigate = useCallback(
    (to: Route) => {
      if (top.kind === "page" && top.route === to && phase === "in") return;
      go([{ kind: "page", route: to }]);
    },
    [top, phase, go],
  );

  const push = useCallback((v: View, skipOut = false) => go([...stack, v], skipOut), [go, stack]);

  const back = useCallback(() => {
    if (fullscreen) {
      setFullscreen(false);
      api.setFullscreen(false).catch(() => {});
    }
    if (stack.length > 1) go(stack.slice(0, -1));
    else if (route !== "home") go([{ kind: "page", route: "home" }]);
  }, [stack, go, route, fullscreen]);

  const openGame = useCallback(
    (gameId: number, from?: Element | null) => {
      const r = from?.getBoundingClientRect();
      const origin = r && r.width > 0 ? { x: r.left, y: r.top, w: r.width, h: r.height } : undefined;
      push({ kind: "game", gameId, origin }, !!origin);
    },
    [push],
  );

  // Controller: LB / RB flip pivot pages, View + Menu opens the game menu
  // (the player listens for F1, same as the keyboard shortcut).
  const shoulder = useRef<(d: -1 | 1) => void>(() => {});
  shoulder.current = (d) => {
    if (top.kind !== "page") return;
    const i = PIVOT.indexOf(route);
    navigate(PIVOT[(Math.max(0, i) + d + PIVOT.length) % PIVOT.length]);
  };
  useEffect(
    () =>
      installGamepad({
        onShoulder: (d) => shoulder.current(d),
        onGameMenu: () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "F1", bubbles: true, cancelable: true })),
      }),
    [],
  );

  const nav = useMemo<Nav>(
    () => ({
      navigate,
      back,
      play: (gameId) => push({ kind: "play", gameId }),
      openGame,
      openCollection: (id) => push({ kind: "collection", id }),
    }),
    [navigate, back, push, openGame],
  );

  const setFs = useCallback((on: boolean) => {
    setFullscreen(on);
    api.setFullscreen(on).catch(() => {});
  }, []);

  // Backspace / Escape goes back, like the hardware back button did.
  // Never while a game is running: the game owns the keyboard then.
  useEffect(() => {
    if (shown.kind === "play") return;
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.tagName === "INPUT" || t.tagName === "TEXTAREA") return;
      if (e.defaultPrevented) return;
      if (e.key === "Backspace" || e.key === "Escape") back();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [back, shown.kind]);

  // A game or collection that got deleted out from under its page.
  useEffect(() => {
    const gone =
      ((shown.kind === "game" || shown.kind === "play") && games.length > 0 && !games.some((g) => g.id === shown.gameId)) ||
      (shown.kind === "collection" && !collections.some((c) => c.id === shown.id));
    if (gone && phase === "in") back();
  }, [shown, games, collections, phase, back]);

  // New SWFs in watched folders, never while a game is running.
  useWatchedFolders(shown.kind === "play");

  const playing = shown.kind === "play" ? games.find((g) => g.id === shown.gameId) : undefined;
  const pageGame = shown.kind === "game" ? games.find((g) => g.id === shown.gameId) : undefined;
  const collection = shown.kind === "collection" ? collections.find((c) => c.id === shown.id) : undefined;
  const page = shown.kind === "page" ? shown.route : null;

  return (
    <NavContext.Provider value={nav}>
      <UpdateProvider paused={shown.kind === "play"}>
      <ImportProvider onImported={() => page === "add" && navigate("library")}>
        <div className={s.app} data-view={shown.kind}>
          {!fullscreen && <TitleBar subtitle={playing?.title} />}
          {shown.kind === "page" && <Pivot route={route} onNavigate={navigate} />}
          <main className={s.main} data-phase={phase} key={`${shown.kind}-${visit}`}>
            {page === "home" && <Home onNavigate={navigate} isSample={!isTauri} />}
            {page === "library" && <Library />}
            {page === "collections" && <Collections />}
            {page === "browse" && <Browse />}
            {page === "add" && <AddGame />}
            {page === "settings" && <Settings />}
            {pageGame && <GamePage game={pageGame} origin={shown.kind === "game" ? shown.origin : undefined} />}
            {collection && <CollectionView collection={collection} />}
            {playing && (
              <div className={`${s.playWrap} ts`}>
                <Player game={playing} fullscreen={fullscreen} onFullscreen={setFs} onExit={back} />
              </div>
            )}
          </main>
        </div>
      </ImportProvider>
      </UpdateProvider>
    </NavContext.Provider>
  );
}
