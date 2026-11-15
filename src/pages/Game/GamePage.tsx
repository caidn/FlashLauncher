import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { ArtPicker } from "../../art/ArtPicker";
import { MatchDialog } from "../../art/MatchDialog";
import { AddToCollection } from "../../components/Collections/AddToCollection";
import { Segmented } from "../../components/Controls/Controls";
import { ConfirmDialog, PromptDialog } from "../../components/Dialog/Dialogs";
import { Icon } from "../../components/Icon/Icon";
import { useLibrary } from "../../data/LibraryProvider";
import type { ArtItem, Compat, Game } from "../../data/types";
import { api } from "../../lib/api";
import { formatBytes, formatFps, formatPlayTime, timeAgo } from "../../lib/format";
import { useNav } from "../../nav";
import { readableOn } from "../../theme/accents";
import { ControllerDialog } from "../../player/ControllerDialog";
import { useReducedMotion } from "../../theme/ThemeProvider";
import s from "./GamePage.module.css";

/** Screen rect of the tile the page was opened from. */
export interface Origin {
  x: number;
  y: number;
  w: number;
  h: number;
}

const SOURCE_NAMES: Record<string, string> = {
  flashpoint: "Flashpoint Archive",
  steamgriddb: "SteamGridDB",
  igdb: "IGDB",
  archive: "Internet Archive",
};

const COMPAT: { value: Compat; label: string }[] = [
  { value: "works", label: "works" },
  { value: "partial", label: "partial" },
  { value: "projector", label: "needs projector" },
  { value: "unknown", label: "untested" },
];

type Dialog = "art" | "match" | "collections" | "rename" | "delete" | "controller" | null;

export function GamePage({ game, origin }: { game: Game; origin?: Origin }) {
  const { play, back, openCollection } = useNav();
  const { updateGame, deleteGames, collections } = useLibrary();
  const [dialog, setDialog] = useState<Dialog>(null);
  const [shots, setShots] = useState<ArtItem[]>([]);
  const [viewer, setViewer] = useState<string | null>(null);
  const hero = useRef<HTMLDivElement>(null);
  const reduced = useReducedMotion();

  const a = game.art ?? {};
  // A real hero banner fills the whole header. Without one, the header
  // splits: flat game color on the left with the logo or title card, and a
  // screenshot on the right, so small images never get blown up to banner size.
  const fullHero = a.hero ?? null;
  const sidePic = fullHero ? null : (a.screenshot ?? a.cover ?? a.embedded ?? null);
  const nameArt = a.logo ?? (fullHero ? null : a.wide) ?? null;
  const inCollections = collections.filter((c) => c.gameIds.includes(game.id));

  useEffect(() => {
    api
      .listArt(game.id)
      .then((items) =>
        setShots(items.filter((i) => i.slot === "screenshot" && (i.path || i.thumbUrl || i.remoteUrl)).slice(0, 12)),
      )
      .catch(() => setShots([]));
  }, [game.id, game.art?.screenshot]);

  // Esc closes the screenshot viewer before it means "back".
  useEffect(() => {
    if (!viewer) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" || e.key === "Backspace") {
        e.preventDefault();
        e.stopPropagation();
        setViewer(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [viewer]);

  // Continuum: the hero starts clipped to the tile that was clicked and
  // opens out to fill its box, so the page grows out of that tile.
  useLayoutEffect(() => {
    const el = hero.current;
    if (!el || !origin || reduced) return;
    const r = el.getBoundingClientRect();
    const top = Math.max(0, origin.y - r.top);
    const left = Math.max(0, origin.x - r.left);
    const right = Math.max(0, r.right - (origin.x + origin.w));
    const bottom = Math.max(0, r.bottom - (origin.y + origin.h));
    el.style.transition = "none";
    el.style.clipPath = `inset(${top}px ${right}px ${bottom}px ${left}px)`;
    el.getBoundingClientRect(); // commit the start state
    requestAnimationFrame(() => {
      el.style.transition = "clip-path 420ms var(--ease-out)";
      el.style.clipPath = "inset(0 0 0 0)";
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const details: [string, React.ReactNode][] = [
    ["developer", game.developer],
    ["publisher", game.publisher],
    ["released", game.releaseDate ?? game.year],
    ["series", game.series],
    ["actionscript", `AS${game.asVersion} · SWF ${game.swfVersion}`],
    ["stage", `${Math.round(game.width)}×${Math.round(game.height)} · ${formatFps(game.frameRate)}`],
    ["file", `${game.originalName} · ${formatBytes(game.fileSize)}`],
    ["came from", game.contentId ? "downloaded from Flashpoint Archive, with its own files" : null],
    ["original address", game.launchUrl],
    ["added", timeAgo(game.addedAt)],
    ["info from", game.links.length ? game.links.map((l) => SOURCE_NAMES[l] ?? l).join(", ") : null],
  ];

  return (
    <div className={s.page}>
      <div
        className={s.hero}
        ref={hero}
        data-mode={fullHero ? "full" : "split"}
        style={{ background: game.color, color: readableOn(game.color) }}
      >
        {fullHero && <img className={s.heroImg} src={api.fileUrl(fullHero)} alt="" draggable={false} />}
        {sidePic && (
          <span className={s.sidePic} data-contain={sidePic === a.embedded}>
            <img src={api.fileUrl(sidePic)} alt="" draggable={false} />
          </span>
        )}
        {!fullHero && !sidePic && (
          <span className={s.heroGlyph}>
            <Icon name="bolt" size={320} fill />
          </span>
        )}
        <button className={s.back} onClick={back} aria-label="Back" data-nav>
          <Icon name="back" size={20} />
        </button>
        <div className={s.heroTitle}>
          {nameArt ? (
            <img
              className={s.logo}
              data-card={nameArt === a.wide}
              src={api.fileUrl(nameArt)}
              alt={game.title}
              draggable={false}
            />
          ) : (
            <h1 className={s.titleText} data-on-image={!!fullHero}>
              {game.title}
            </h1>
          )}
        </div>
      </div>

      <div className={`${s.bar} ts`} style={{ ["--i" as string]: 1 }}>
        <button className={s.play} onClick={() => play(game.id)} data-nav autoFocus>
          <Icon name="play" size={22} fill />
          <span>play</span>
        </button>
        <div className={s.stats}>
          <span className={s.statBig}>{game.playSeconds ? formatPlayTime(game.playSeconds) : "not played yet"}</span>
          <span className={s.statSmall}>{game.lastPlayed ? `last played ${timeAgo(game.lastPlayed)}` : "added " + timeAgo(game.addedAt)}</span>
        </div>
        <span className={s.spacer} />
        <div className={s.actions}>
          <Action icon="star" label={game.favorite ? "favorited" : "favorite"} active={game.favorite} onClick={() => updateGame(game.id, { favorite: !game.favorite })} />
          <Action icon="grid" label="collections" onClick={() => setDialog("collections")} />
          <Action icon="image" label="art" onClick={() => setDialog("art")} />
          <Action icon="search" label="find info" onClick={() => setDialog("match")} />
          <Action icon="gamepad" label="controller" onClick={() => setDialog("controller")} />
          <Action icon="edit" label="rename" onClick={() => setDialog("rename")} />
          <Action icon="trash" label="delete" onClick={() => setDialog("delete")} />
        </div>
      </div>

      <div className={s.body}>
        <div className={`${s.main} ts`} style={{ ["--i" as string]: 2 }}>
          {game.description ? (
            <p className={s.description}>{game.description}</p>
          ) : (
            <p className={s.noDescription}>
              No description yet.{" "}
              <button onClick={() => setDialog("match")} className={s.link}>
                find info
              </button>{" "}
              to look one up.
            </p>
          )}

          {shots.length > 0 && (
            <section className={s.section}>
              <h2 className={s.sectionTitle}>screenshots</h2>
              <div className={s.shots}>
                {shots.map((sh) => {
                  const src = sh.path ? api.fileUrl(sh.path) : (sh.remoteUrl ?? sh.thumbUrl ?? "");
                  return (
                    <button key={sh.id} className={s.shot} onClick={() => setViewer(src)} data-nav aria-label="View screenshot">
                      <img src={sh.path ? src : (sh.thumbUrl ?? src)} alt="" loading="lazy" draggable={false} />
                    </button>
                  );
                })}
              </div>
            </section>
          )}

          {game.tags.length > 0 && (
            <section className={s.section}>
              <h2 className={s.sectionTitle}>tags</h2>
              <div className={s.tags}>
                {game.tags.map((t) => (
                  <span key={t} className={s.tag}>
                    {t}
                  </span>
                ))}
              </div>
            </section>
          )}
        </div>

        <aside className={`${s.side} ts`} style={{ ["--i" as string]: 3 }}>
          <dl className={s.details}>
            {details
              .filter(([, v]) => v !== null && v !== undefined && v !== "")
              .map(([k, v]) => (
                <div key={k} className={s.detail}>
                  <dt>{k}</dt>
                  <dd>{v}</dd>
                </div>
              ))}
          </dl>

          <section className={s.section}>
            <h2 className={s.sectionTitle}>runs in ruffle?</h2>
            <Segmented<Compat> label="Compatibility" value={game.compat} onChange={(v) => updateGame(game.id, { compat: v })} options={COMPAT} />
          </section>

          <section className={s.section}>
            <h2 className={s.sectionTitle}>collections</h2>
            {inCollections.length ? (
              <div className={s.tags}>
                {inCollections.map((c) => (
                  <button key={c.id} className={s.collection} onClick={() => openCollection(c.id)} data-nav>
                    {c.name}
                  </button>
                ))}
              </div>
            ) : (
              <p className={s.muted}>
                Not in any.{" "}
                <button onClick={() => setDialog("collections")} className={s.link}>
                  add it to one
                </button>
              </p>
            )}
          </section>
        </aside>
      </div>

      {viewer && (
        <button className={s.viewer} onClick={() => setViewer(null)} aria-label="Close screenshot">
          <img src={viewer} alt="" />
        </button>
      )}

      {dialog === "art" && <ArtPicker game={game} onClose={() => setDialog(null)} onFindMatch={() => setDialog("match")} />}
      {dialog === "match" && <MatchDialog game={game} onClose={() => setDialog(null)} />}
      {dialog === "controller" && <ControllerDialog game={game} onClose={() => setDialog(null)} />}
      {dialog === "collections" && <AddToCollection gameIds={[game.id]} onClose={() => setDialog(null)} />}
      {dialog === "rename" && (
        <PromptDialog
          title="rename"
          note={game.originalName}
          label="Title"
          initial={game.title}
          onClose={() => setDialog(null)}
          onSave={(title) => {
            updateGame(game.id, { title });
            setDialog(null);
          }}
        />
      )}
      {dialog === "delete" && (
        <ConfirmDialog
          title={`delete ${game.title}?`}
          confirmLabel="delete"
          onClose={() => setDialog(null)}
          onConfirm={() => {
            setDialog(null);
            deleteGames([game.id]);
          }}
        >
          <p>
            This removes it from your library along with the launcher's copy and any art. Your saves are kept in case
            you add the same file again. The original file isn't touched.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}

function Action({
  icon,
  label,
  onClick,
  active = false,
}: {
  icon: "star" | "grid" | "image" | "search" | "edit" | "trash" | "gamepad";
  label: string;
  onClick: () => void;
  active?: boolean;
}) {
  return (
    <button className={s.action} onClick={onClick} data-active={active} data-nav>
      <span className={s.actionGlyph}>
        <Icon name={icon} size={18} fill={icon === "star" && active} />
      </span>
      <span>{label}</span>
    </button>
  );
}
