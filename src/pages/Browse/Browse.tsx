import { useCallback, useEffect, useRef, useState } from "react";
import { useDownloads, progressText } from "../../browse/DownloadProvider";
import { Button, Segmented } from "../../components/Controls/Controls";
import { BandDialog } from "../../components/Dialog/BandDialog";
import { ProgressBar, ProgressDots } from "../../components/Feedback/Feedback";
import { Icon } from "../../components/Icon/Icon";
import { useLibrary } from "../../data/LibraryProvider";
import type { BrowseGame, BrowseKind, BrowseSort } from "../../data/types";
import { api } from "../../lib/api";
import { formatBytes } from "../../lib/format";
import { useTilt } from "../../lib/useTilt";
import { useNav } from "../../nav";
import lib from "../Library/Library.module.css";
import s from "./Browse.module.css";

const SORTS: { value: BrowseSort; label: string }[] = [
  { value: "new", label: "recently added" },
  { value: "title", label: "a-z" },
  { value: "newest", label: "newest" },
  { value: "oldest", label: "oldest" },
];

const KINDS: { value: BrowseKind; label: string }[] = [
  { value: "games", label: "games" },
  { value: "animations", label: "animations" },
];

const TILE_COLORS = ["#00aba9", "#1ba1e2", "#0050ef", "#6a00ff", "#d80073", "#e51400", "#fa6800", "#60a917"];

function colorFor(id: string): string {
  let h = 0;
  for (const c of id) h = (h * 31 + c.charCodeAt(0)) | 0;
  return TILE_COLORS[Math.abs(h) % TILE_COLORS.length];
}

/** Browse and download from Flashpoint Archive's Flash collection. */
export function Browse() {
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [sort, setSort] = useState<BrowseSort>("new");
  const [kind, setKind] = useState<BrowseKind>("games");
  const [games, setGames] = useState<BrowseGame[]>([]);
  const [page, setPage] = useState(1);
  const [pages, setPages] = useState(0);
  const [total, setTotal] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<BrowseGame | null>(null);
  const [attempt, setAttempt] = useState(0);
  const req = useRef(0);

  useEffect(() => {
    const t = window.setTimeout(() => setDebounced(query.trim()), 350);
    return () => window.clearTimeout(t);
  }, [query]);

  const load = useCallback(
    async (p: number) => {
      const mine = ++req.current;
      setLoading(true);
      setError(null);
      try {
        const r = await api.browse(debounced, p, sort, kind);
        if (mine !== req.current) return;
        setGames((prev) => (p === 1 ? r.games : [...prev, ...r.games.filter((g) => !prev.some((x) => x.id === g.id))]));
        setPage(r.page || p);
        setPages(r.pages);
        setTotal(r.total);
      } catch (e) {
        if (mine === req.current) setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (mine === req.current) setLoading(false);
      }
    },
    [debounced, sort, kind],
  );

  useEffect(() => {
    load(1);
  }, [load, attempt]);

  return (
    <div className={lib.page}>
      <div className={`${lib.toolbar} ts`} style={{ ["--i" as string]: 0 }}>
        <label className={lib.search}>
          <Icon name="search" size={18} />
          <input
            placeholder="search flashpoint"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && setDebounced(query.trim())}
            data-nav
            autoFocus
          />
        </label>
        <Segmented label="Show" value={kind} onChange={setKind} options={KINDS} />
        <span className={lib.spacer} />
        <Segmented label="Sort" value={sort} onChange={setSort} options={SORTS} />
      </div>

      <div className={lib.count}>
        <span>
          {total === null
            ? "looking…"
            : debounced
              ? `${total.toLocaleString()} ${total === 1 ? "match" : "matches"} in Flashpoint`
              : `${total.toLocaleString()} Flash games and animations in Flashpoint Archive`}
        </span>
        <span className={s.credit}>
          Games are preserved by{" "}
          <a
            href="https://flashpointarchive.org"
            onClick={(e) => {
              e.preventDefault();
              api.openExternal("https://flashpointarchive.org").catch(() => {});
            }}
          >
            Flashpoint Archive
          </a>
          , a volunteer project. Downloads come from their servers.
        </span>
      </div>

      <div className={s.scroll}>
        {error && games.length === 0 ? (
          <div className={s.message}>
            <h2>couldn't reach Flashpoint</h2>
            <p>{error}</p>
            <Button variant="accent" onClick={() => setAttempt((a) => a + 1)}>
              try again
            </Button>
          </div>
        ) : !loading && games.length === 0 && page >= pages ? (
          <div className={s.message}>
            <h2>nothing found</h2>
            <p>Try fewer words, or the game's original name.</p>
          </div>
        ) : (
          <div className={s.grid}>
            {games.map((g, i) => (
              <ResultTile key={g.id} game={g} index={i % 30} onOpen={() => setOpen(g)} />
            ))}
          </div>
        )}
        <div className={s.more}>
          {loading ? (
            <ProgressDots label="Loading results" />
          ) : (
            page < pages && (
              <Button onClick={() => load(page + 1)}>more</Button>
            )
          )}
          {error && games.length > 0 && <span className={s.moreError}>{error}</span>}
        </div>
      </div>

      {open && <GameDialog game={open} onClose={() => setOpen(null)} />}
    </div>
  );
}

function useStatus(g: BrowseGame) {
  const { games } = useLibrary();
  const { current, queued, done, failed } = useDownloads();
  const libraryId = done[g.id] ?? g.libraryId;
  const inLibrary = libraryId != null && games.some((x) => x.id === libraryId);
  return {
    inLibrary,
    libraryId,
    active: current?.id === g.id ? current : null,
    queued: queued.includes(g.id),
    failed: failed[g.id],
  };
}

function Logo({ game, thumb = true }: { game: BrowseGame; thumb?: boolean }) {
  const [broken, setBroken] = useState(false);
  const src = thumb ? (game.logoThumb ?? game.logo) : (game.logo ?? game.logoThumb);
  if (!src || broken) {
    return (
      <span className={s.placeholder} style={{ background: colorFor(game.id) }}>
        {game.title}
      </span>
    );
  }
  return <img className={s.logo} src={src} alt="" loading="lazy" onError={() => setBroken(true)} />;
}

function ResultTile({ game, index, onOpen }: { game: BrowseGame; index: number; onOpen: () => void }) {
  const tilt = useTilt<HTMLButtonElement>();
  const st = useStatus(game);
  return (
    <div className="stagger" style={{ ["--i" as string]: index }}>
      <button {...tilt} className={s.tile} onClick={onOpen} data-nav aria-label={game.title}>
        <span className={s.art}>
          <Logo game={game} />
          {st.inLibrary && <span className={s.badge}>in library</span>}
          {st.active && (
            <span className={s.tileProgress}>
              <ProgressBar value={st.active.progress?.received ?? 0} max={st.active.progress?.total || 1} />
            </span>
          )}
          {st.queued && <span className={s.badge}>queued</span>}
        </span>
        <span className={s.meta}>
          <span className={s.title}>{game.title}</span>
          <span className={s.sub}>{[game.developer, game.year].filter(Boolean).join(" · ") || "unknown"}</span>
        </span>
      </button>
    </div>
  );
}

function GameDialog({ game, onClose }: { game: BrowseGame; onClose: () => void }) {
  const { download, cancel } = useDownloads();
  const { openGame, play } = useNav();
  const st = useStatus(game);
  const [shotBroken, setShotBroken] = useState(false);
  const busy = !!st.active || st.queued;

  const actions = st.inLibrary ? (
    <>
      <Button
        variant="accent"
        onClick={() => {
          onClose();
          play(st.libraryId!);
        }}
        data-autofocus
      >
        <Icon name="play" size={14} /> play
      </Button>
      <Button
        onClick={() => {
          onClose();
          openGame(st.libraryId!);
        }}
      >
        open in library
      </Button>
      <Button onClick={onClose}>close</Button>
    </>
  ) : busy ? (
    <>
      <Button onClick={cancel} disabled={!st.active}>
        cancel download
      </Button>
      <Button onClick={onClose}>keep browsing</Button>
    </>
  ) : (
    <>
      <Button variant="accent" onClick={() => download(game)} data-autofocus>
        <Icon name="download" size={14} /> {st.failed ? "try again" : "download"}
      </Button>
      <Button onClick={onClose}>close</Button>
    </>
  );

  return (
    <BandDialog title={game.title} onCancel={onClose} actions={actions} wide tall>
      <div className={s.detail}>
        <div className={s.detailArt}>
          {game.screenshot && !shotBroken ? (
            <img src={game.screenshot} alt="" onError={() => setShotBroken(true)} />
          ) : (
            <Logo game={game} thumb={false} />
          )}
        </div>
        <div className={s.detailText}>
          <dl className={s.facts}>
            {game.developer && (
              <>
                <dt>developer</dt>
                <dd>{game.developer}</dd>
              </>
            )}
            {game.publisher && game.publisher !== game.developer && (
              <>
                <dt>publisher</dt>
                <dd>{game.publisher}</dd>
              </>
            )}
            {(game.releaseDate || game.year) && (
              <>
                <dt>released</dt>
                <dd>{game.releaseDate ?? game.year}</dd>
              </>
            )}
            {game.series && (
              <>
                <dt>series</dt>
                <dd>{game.series}</dd>
              </>
            )}
            <dt>download</dt>
            <dd>{game.size != null ? formatBytes(game.size) : "fetched as you play"}</dd>
          </dl>
          {game.tags.length > 0 && (
            <div className={s.tags}>
              {game.tags.slice(0, 10).map((t) => (
                <span key={t}>{t.toLowerCase()}</span>
              ))}
            </div>
          )}
          {game.description && <p className={s.description}>{game.description}</p>}
          {game.launchKind === "page" && !st.inLibrary && (
            <p className={s.note}>
              This one starts from a web page instead of a SWF. The launcher pulls out the game's SWF, but some of these
              need the page around them and won't run right.
            </p>
          )}
          {st.active && (
            <div className={s.status}>
              <span>{progressText(st.active.progress)}</span>
              <ProgressBar value={st.active.progress?.received ?? 0} max={st.active.progress?.total || 1} />
            </div>
          )}
          {st.queued && <p className={s.note}>Queued. It starts when the current download finishes.</p>}
          {st.failed && !busy && <p className={s.error}>{st.failed}</p>}
        </div>
      </div>
    </BandDialog>
  );
}
