import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { Button } from "../components/Controls/Controls";
import { ProgressDots, useToast } from "../components/Feedback/Feedback";
import { useLibrary } from "../data/LibraryProvider";
import type { ArtItem, Game, GamePatch } from "../data/types";
import { api } from "../lib/api";
import { formatPlayTime } from "../lib/format";
import { setSpatialNavEnabled } from "../lib/spatialNav";
import { Overlay } from "./Overlay";
import { padMapFor } from "./gamepadMap";
import { captureFrame, loadRuffle, type RuffleApi, type RuffleConfig, type RufflePlayerElement } from "./ruffle";
import { clearGameStorage, restoreSaves, snapshotSaves } from "./saves";
import s from "./Player.module.css";

type Phase =
  | { kind: "loading" }
  | { kind: "playing" }
  | { kind: "projector"; since: number }
  | { kind: "error"; message: string };

const SAVE_EVERY_MS = 20_000;
const SESSION_EVERY_S = 15;

interface Props {
  game: Game;
  fullscreen: boolean;
  onFullscreen: (on: boolean) => void;
  onExit: () => void;
}

export function Player({ game, fullscreen, onFullscreen, onExit }: Props) {
  const { updateGame, reloadGame } = useLibrary();
  const toast = useToast();
  const stage = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLDivElement>(null);
  const player = useRef<RufflePlayerElement | null>(null);
  const ruffle = useRef<RuffleApi | null>(null);
  const session = useRef<{ id: number; seconds: number } | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "loading" });
  const [menu, setMenu] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [hint, setHint] = useState(true);
  const [shot, setShot] = useState<ArtItem | null>(null);
  const [volume, setVolume] = useState(80);
  const [muted, setMuted] = useState(false);
  // Settings that only apply when the game (re)starts.
  const [pendingRestart, setPendingRestart] = useState(false);

  const gameRef = useRef(game);
  gameRef.current = game;

  // ------------------------------------------------------------ persistence

  const flush = useCallback(async () => {
    await persist(gameRef.current.id, ruffle.current ? snapshotSaves() : null, session.current);
  }, []);

  // ------------------------------------------------------------ start / stop

  useEffect(() => {
    let cancelled = false;
    let offProjector = () => {};
    setPhase({ kind: "loading" });
    setPendingRestart(false);

    (async () => {
      try {
        const settings = await api.getSettings().catch(() => ({}) as Record<string, string>);
        const vol = settings.volume !== undefined ? Number(settings.volume) : 80;
        const mute = settings.muted === "true";
        setVolume(vol);
        setMuted(mute);

        if (game.runtime === "projector") {
          offProjector = await api.onProjectorExit((e) => {
            if (e.gameId !== game.id) return;
            if (e.error) toast({ title: "the projector had a problem", body: e.error });
            reloadGame(game.id);
            onExit();
          });
          await api.launchProjector(game.id);
          if (!cancelled) setPhase({ kind: "projector", since: Date.now() });
          return;
        }

        // Downloaded games carry their own files: load the SWF from where it
        // originally lived and send every address it asks for to our
        // fpcontent:// copy, so its levels, sounds and XML all resolve
        // offline. Uploaded games are a single file, handed over as bytes.
        const content = game.contentId && game.launchUrl ? api.contentBase(game.contentId) : null;
        const [bytes, saves, R] = await Promise.all([
          content ? Promise.resolve(null) : api.readSwf(game.id),
          api.getSaves(game.id),
          loadRuffle(),
        ]);
        if (cancelled) return;
        restoreSaves(saves);

        const el = R.newest().createPlayer();
        el.classList.add(s.ruffle);
        // Ruffle shows a big play button whenever it's suspended. Our menu
        // already says "paused", so hide it.
        const style = document.createElement("style");
        style.textContent = "#play-button{display:none!important}";
        el.shadowRoot?.appendChild(style);
        host.current?.replaceChildren(el);
        player.current = el;
        const r = el.ruffle();
        ruffle.current = r;
        const config = {
          autoplay: "on",
          unmuteOverlay: "hidden",
          splashScreen: false,
          preloader: false,
          letterbox: "off",
          // We size the element ourselves for fit / stretch / original, so
          // Ruffle always just fills it.
          scale: "exactFit",
          forceScale: true,
          quality: game.quality,
          frameRate: game.frameRateOverride,
          // Downloaded games need loading turned on to reach their own files;
          // the rewrite below keeps those loads local.
          allowNetworking: game.allowNetwork ? "all" : content ? "internal" : "none",
          openUrlMode: game.allowNetwork ? "confirm" : "deny",
          contextMenu: "rightClickOnly",
          warnOnUnsupportedContent: false,
          showSwfDownload: false,
          allowFullscreen: true,
          logLevel: "error",
          gamepadButtonMapping: padMapFor(game),
        } satisfies RuffleConfig;
        if (content) {
          await r.load({
            ...config,
            url: game.launchUrl!,
            urlRewriteRules: [[/^https?:\/\/([^?#]*)(?:[?#].*)?$/, `${content}$1`]],
          });
        } else {
          await r.load({ ...config, data: bytes!, swfFileName: game.originalName || "game.swf" });
        }
        if (cancelled) return;
        r.volume = mute ? 0 : vol / 100;
        const id = await api.sessionStart(game.id);
        session.current = { id, seconds: 0 };
        setPhase({ kind: "playing" });
        el.focus();
      } catch (e) {
        if (!cancelled) setPhase({ kind: "error", message: e instanceof Error ? e.message : String(e) });
      }
    })();

    return () => {
      cancelled = true;
      offProjector();
      // Snapshot and clear synchronously: the next game (or a restart of
      // this one) restores its own saves right after this runs.
      const id = game.id;
      const snap = ruffle.current ? snapshotSaves() : null;
      const sess = session.current ? { ...session.current } : null;
      session.current = null;
      ruffle.current = null;
      player.current?.remove();
      player.current = null;
      clearGameStorage();
      if (sess) api.discordClear().catch(() => {});
      persist(id, snap, sess).finally(() => reloadGame(id));
    };
    // game.id + attempt restart the game; other game fields are read at start.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id, attempt]);

  // Saves and play time while running, and before the window closes.
  useEffect(() => {
    const t = window.setInterval(flush, SAVE_EVERY_MS);
    let off = () => {};
    api.onBeforeClose(flush).then((fn) => (off = fn));
    return () => {
      window.clearInterval(t);
      off();
    };
  }, [flush]);

  // Count play time: only while the game is actually running and visible.
  useEffect(() => {
    if (phase.kind !== "playing") return;
    const t = window.setInterval(() => {
      const sess = session.current;
      if (!sess || menu || document.visibilityState !== "visible") return;
      sess.seconds += 1;
      if (sess.seconds % SESSION_EVERY_S === 0) api.sessionUpdate(sess.id, sess.seconds).catch(() => {});
    }, 1000);
    return () => window.clearInterval(t);
  }, [phase.kind, menu]);

  // ------------------------------------------------------------ layout

  // Ruffle fills its element; we size the element for the scale mode.
  useLayoutEffect(() => {
    const st = stage.current;
    const h = host.current;
    if (!st || !h) return;
    const w0 = game.width > 0 ? game.width : 550;
    const h0 = game.height > 0 ? game.height : 400;
    const fit = () => {
      const cw = st.clientWidth;
      const ch = st.clientHeight;
      let w = cw;
      let hh = ch;
      if (game.scaleMode === "fit") {
        const k = Math.min(cw / w0, ch / h0);
        w = Math.floor(w0 * k);
        hh = Math.floor(h0 * k);
      } else if (game.scaleMode === "original") {
        w = w0;
        hh = h0;
      }
      h.style.width = `${w}px`;
      h.style.height = `${hh}px`;
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(st);
    return () => ro.disconnect();
  }, [game.scaleMode, game.width, game.height]);

  // ------------------------------------------------------------ controls

  const openMenu = useCallback(() => {
    setMenu(true);
    setHint(false);
    ruffle.current?.suspend();
    flush();
  }, [flush]);

  const closeMenu = useCallback(() => {
    setMenu(false);
    ruffle.current?.resume();
    player.current?.focus();
  }, []);

  useEffect(() => {
    setSpatialNavEnabled(menu || phase.kind !== "playing");
    return () => setSpatialNavEnabled(true);
  }, [menu, phase.kind]);

  // The key hint shows for a few seconds once the game is actually up.
  const running = phase.kind === "playing";
  useEffect(() => {
    if (!running) return;
    setHint(true);
    const t = window.setTimeout(() => setHint(false), 6000);
    return () => window.clearTimeout(t);
  }, [running, attempt]);

  const takeScreenshot = useCallback(async () => {
    const el = player.current;
    if (!el) return;
    try {
      // The overlay covers the game; close it and let a frame draw first.
      if (menu) closeMenu();
      await new Promise((r) => setTimeout(r, 120));
      const png = await captureFrame(el);
      const item = await api.addScreenshot(gameRef.current.id, png);
      setShot(item);
    } catch (e) {
      toast({ title: "screenshot failed", body: String(e) });
    }
  }, [menu, closeMenu, toast]);

  // Esc / F1 menu, F11 fullscreen, F12 screenshot.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (phase.kind !== "playing") {
        if (e.key === "Escape" && phase.kind === "error") onExit();
        return;
      }
      if (e.key === "F1" || (e.key === "Escape" && !menu)) {
        e.preventDefault();
        if (menu) closeMenu();
        else openMenu();
      } else if (e.key === "Escape" && menu) {
        e.preventDefault();
        closeMenu();
      } else if (e.key === "F11") {
        e.preventDefault();
        onFullscreen(!fullscreen);
      } else if (e.key === "F12") {
        e.preventDefault();
        takeScreenshot();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [phase.kind, menu, openMenu, closeMenu, fullscreen, onFullscreen, takeScreenshot, onExit]);

  useEffect(() => {
    if (!shot) return;
    const t = window.setTimeout(() => setShot(null), 7000);
    return () => window.clearTimeout(t);
  }, [shot]);

  const setVol = (v: number) => {
    setVolume(v);
    if (ruffle.current) ruffle.current.volume = muted ? 0 : v / 100;
    api.setSetting("volume", String(v)).catch(() => {});
  };

  const setMute = (m: boolean) => {
    setMuted(m);
    if (ruffle.current) ruffle.current.volume = m ? 0 : volume / 100;
    api.setSetting("muted", String(m)).catch(() => {});
  };

  const patch = (p: GamePatch, needsRestart: boolean) => {
    updateGame(game.id, p);
    if (needsRestart) setPendingRestart(true);
  };

  const restart = async () => {
    await flush();
    setMenu(false);
    setAttempt((a) => a + 1);
  };

  const applyShotAsArt = async (item: ArtItem) => {
    await api.selectArt(game.id, item.id);
    await reloadGame(game.id);
    setShot(null);
    toast({ title: "tile art updated", body: `${game.title} uses that screenshot now.` });
  };

  // ------------------------------------------------------------ render

  return (
    <div className={s.player} data-fullscreen={fullscreen}>
      <div className={s.stage} ref={stage} style={{ background: game.letterbox ?? "#000" }}>
        <div className={s.host} ref={host} data-hidden={phase.kind !== "playing"} />
      </div>

      {phase.kind === "loading" && (
        <div className={s.cover}>
          <span className={s.coverLabel}>starting</span>
          <h1 className={s.coverTitle}>{game.title}</h1>
          <div className={s.coverDots}>
            <ProgressDots label="Loading game" />
          </div>
        </div>
      )}

      {phase.kind === "projector" && (
        <ProjectorScreen game={game} since={phase.since} />
      )}

      {phase.kind === "error" && (
        <div className={s.cover}>
          <span className={s.coverLabel}>couldn't start</span>
          <h1 className={s.coverTitle}>{game.title}</h1>
          <p className={s.coverText}>{phase.message}</p>
          <div className={s.coverButtons}>
            <Button variant="accent" onClick={() => setAttempt((a) => a + 1)}>
              try again
            </Button>
            <Button onClick={onExit}>back</Button>
          </div>
        </div>
      )}

      {phase.kind === "playing" && hint && !menu && (
        <div className={s.hint}>
          <kbd>esc</kbd> or <kbd>F1</kbd> for the menu
          {Array.from(navigator.getGamepads?.() ?? []).some((p) => p?.connected) && (
            <>
              {" "}
              · <kbd>view</kbd> + <kbd>menu</kbd> on a controller
            </>
          )}
        </div>
      )}

      {shot && !menu && (
        <div className={s.shot}>
          <img src={api.fileUrl(shot.path)} alt="" />
          <div className={s.shotText}>
            <span className={s.shotTitle}>screenshot saved</span>
            <button className={s.shotAction} onClick={() => applyShotAsArt(shot)}>
              use as tile art
            </button>
          </div>
        </div>
      )}

      {menu && phase.kind === "playing" && (
        <Overlay
          game={game}
          fullscreen={fullscreen}
          volume={volume}
          muted={muted}
          pendingRestart={pendingRestart}
          onResume={closeMenu}
          onScreenshot={takeScreenshot}
          onFullscreen={() => onFullscreen(!fullscreen)}
          onRestart={restart}
          onQuit={onExit}
          onVolume={setVol}
          onMute={setMute}
          onPatch={patch}
        />
      )}
    </div>
  );
}

async function persist(gameId: number, saves: string | null, sess: { id: number; seconds: number } | null) {
  const tasks: Promise<unknown>[] = [];
  if (saves !== null) tasks.push(api.putSaves(gameId, saves).catch(() => {}));
  if (sess) tasks.push(api.sessionUpdate(sess.id, sess.seconds).catch(() => {}));
  await Promise.all(tasks);
}

function ProjectorScreen({ game, since }: { game: Game; since: number }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, []);
  const secs = Math.floor((now - since) / 1000);
  return (
    <div className={s.cover}>
      <span className={s.coverLabel}>playing in the flash player projector</span>
      <h1 className={s.coverTitle}>{game.title}</h1>
      <p className={s.coverText}>
        {secs < 60 ? `${secs}s` : formatPlayTime(secs)} so far. Close the projector window to come back here, your play
        time gets saved then.
      </p>
    </div>
  );
}
