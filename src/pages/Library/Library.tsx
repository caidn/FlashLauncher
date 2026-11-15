import { useEffect, useMemo, useState } from "react";
import { AppBar, AppBarCommand } from "../../components/AppBar/AppBar";
import { Button, Segmented } from "../../components/Controls/Controls";
import { AddToCollection } from "../../components/Collections/AddToCollection";
import { ConfirmDialog, PromptDialog } from "../../components/Dialog/Dialogs";
import { GameCover, GameGrid } from "../../components/GameCover/GameCover";
import { Icon } from "../../components/Icon/Icon";
import { useLibrary } from "../../data/LibraryProvider";
import type { Game } from "../../data/types";
import { useImport } from "../../import/ImportProvider";
import { ArtPicker } from "../../art/ArtPicker";
import { MatchDialog } from "../../art/MatchDialog";
import { useScrape } from "../../art/ScrapeProvider";
import { useNav } from "../../nav";
import { formatBytes, formatPlayTime, timeAgo } from "../../lib/format";
import s from "./Library.module.css";

type Sort = "recent" | "az" | "played" | "added";
type Filter = "all" | "uploaded" | "archive" | "favorites";
type View = "grid" | "list";

const SORTS: { value: Sort; label: string }[] = [
  { value: "recent", label: "recent" },
  { value: "az", label: "a-z" },
  { value: "played", label: "most played" },
  { value: "added", label: "date added" },
];

const FILTERS: { value: Filter; label: string }[] = [
  { value: "all", label: "all" },
  { value: "uploaded", label: "uploaded" },
  { value: "archive", label: "archive" },
  { value: "favorites", label: "favorites" },
];

const COMPAT_LABEL: Record<Game["compat"], string> = {
  works: "works",
  partial: "partial",
  projector: "needs projector",
  unknown: "untested",
};

function sortGames(games: Game[], sort: Sort): Game[] {
  const g = [...games];
  switch (sort) {
    case "az":
      return g.sort((a, b) => a.title.localeCompare(b.title));
    case "played":
      return g.sort((a, b) => b.playSeconds - a.playSeconds);
    case "added":
      return g.sort((a, b) => b.addedAt.localeCompare(a.addedAt));
    case "recent":
      // Played games by last played, then never-played ones newest first.
      return g.sort(
        (a, b) =>
          (b.lastPlayed ?? "").localeCompare(a.lastPlayed ?? "") || b.addedAt.localeCompare(a.addedAt),
      );
  }
}

export function Library() {
  const { games, loaded, updateGame, deleteGames } = useLibrary();
  const { browseFiles } = useImport();
  const { play, openGame } = useNav();
  const { scrapeGames, running: scraping } = useScrape();
  const [sort, setSort] = useState<Sort>("recent");
  const [filter, setFilter] = useState<Filter>("all");
  const [view, setView] = useState<View>("grid");
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [dialog, setDialog] = useState<"rename" | "delete" | "art" | "match" | "collections" | null>(null);
  // Games to step through in the match dialog ("review" from the banner).
  const [reviewQueue, setReviewQueue] = useState<number[]>([]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    const filtered = games.filter((g) => {
      if (filter === "favorites" && !g.favorite) return false;
      if ((filter === "uploaded" || filter === "archive") && g.source !== filter) return false;
      if (!q) return true;
      return (
        g.title.toLowerCase().includes(q) ||
        (g.developer ?? "").toLowerCase().includes(q) ||
        g.originalName.toLowerCase().includes(q) ||
        g.tags.some((t) => t.includes(q))
      );
    });
    return sortGames(filtered, sort);
  }, [games, sort, filter, query]);

  // Drop selections for games that went away.
  useEffect(() => {
    setSelected((prev) => {
      const ids = new Set(games.map((g) => g.id));
      const next = new Set([...prev].filter((id) => ids.has(id)));
      return next.size === prev.size ? prev : next;
    });
  }, [games]);

  // Esc clears the selection before it means "go home".
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && selected.size && !dialog) {
        e.preventDefault();
        setSelected(new Set());
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selected, dialog]);

  const toggle = (id: number) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

  // Click plays. Right-click (or the menu key) selects, and once anything is
  // selected, clicks select too, the way Windows 8 lists worked.
  const activate = (id: number, el?: Element | null) => (selected.size ? toggle(id) : openGame(id, el));

  const picked = games.filter((g) => selected.has(g.id));
  // Looked up, but no source matched with confidence.
  const unmatched = games.filter((g) => g.scrapedAt && g.links.length === 0);
  const neverScraped = games.filter((g) => !g.scrapedAt).length;
  const reviewing = reviewQueue.length ? games.find((g) => g.id === reviewQueue[0]) : undefined;
  const allFav = picked.length > 0 && picked.every((g) => g.favorite);

  if (loaded && games.length === 0) {
    return <EmptyLibrary onAdd={browseFiles} />;
  }

  return (
    <div className={s.page}>
      <div className={`${s.toolbar} ts`} style={{ ["--i" as string]: 0 }}>
        <label className={s.search}>
          <Icon name="search" size={18} />
          <input
            placeholder="search library"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            data-nav
          />
        </label>
        <Segmented label="Filter" value={filter} onChange={setFilter} options={FILTERS} />
        <span className={s.spacer} />
        <Segmented label="Sort" value={sort} onChange={setSort} options={SORTS} />
        <Segmented
          label="View"
          value={view}
          onChange={setView}
          options={[
            { value: "grid", label: <Icon name="grid" size={16} />, title: "Grid" },
            { value: "list", label: <Icon name="list" size={16} />, title: "List" },
          ]}
        />
      </div>

      <div className={s.count}>
        <span>
          {shown.length} {shown.length === 1 ? "game" : "games"}
          {selected.size > 0 && ` · ${selected.size} selected`}
        </span>
        {!scraping && unmatched.length > 0 && (
          <span className={s.notice}>
            {unmatched.length} {unmatched.length === 1 ? "game wasn't" : "games weren't"} matched automatically
            <button onClick={() => setReviewQueue(unmatched.map((g) => g.id))} data-nav>
              review
            </button>
          </span>
        )}
        {!scraping && neverScraped > 0 && (
          <span className={s.notice}>
            {neverScraped} without art or info yet
            <button onClick={() => scrapeGames(null)} data-nav>
              find it
            </button>
          </span>
        )}
      </div>

      <div className={s.scroll} data-bar={selected.size > 0}>
        {shown.length === 0 ? (
          <p className={s.empty}>nothing matches that.</p>
        ) : view === "grid" ? (
          <GameGrid>
            {shown.map((g, i) => (
              <GameCover
                key={g.id}
                game={g}
                index={i}
                selected={selected.has(g.id)}
                onActivate={(el) => activate(g.id, el)}
                onToggle={() => toggle(g.id)}
              />
            ))}
          </GameGrid>
        ) : (
          <div className={s.list} role="table">
            <div className={s.listHead} role="row">
              <span />
              <span>title</span>
              <span>developer</span>
              <span>as</span>
              <span>stage</span>
              <span>size</span>
              <span>compatibility</span>
              <span>played</span>
              <span>last played</span>
            </div>
            {shown.map((g, i) => (
              <button
                key={g.id}
                className={`${s.row} stagger`}
                style={{ ["--i" as string]: i }}
                role="row"
                aria-selected={selected.has(g.id)}
                data-selected={selected.has(g.id)}
                data-nav
                onClick={(e) => activate(g.id, e.currentTarget)}
                onContextMenu={(e) => {
                  e.preventDefault();
                  toggle(g.id);
                }}
              >
                <span className={s.swatch} style={{ background: g.color }} />
                <span className={s.rowTitle}>
                  {g.title}
                  {g.favorite && <Icon name="star" size={12} fill />}
                </span>
                <span>{g.developer ?? "-"}</span>
                <span>{g.asVersion}</span>
                <span>
                  {Math.round(g.width)}×{Math.round(g.height)}
                </span>
                <span>{formatBytes(g.fileSize)}</span>
                <span data-compat={g.compat} className={s.compat}>
                  {COMPAT_LABEL[g.compat]}
                </span>
                <span>{g.playSeconds ? formatPlayTime(g.playSeconds) : "-"}</span>
                <span>{timeAgo(g.lastPlayed)}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <AppBar
        open={selected.size > 0}
        left={
          <>
            <span>{picked.length === 1 ? picked[0].title : `${picked.length} games`}</span>
            <Button onClick={() => setSelected(new Set())}>clear</Button>
          </>
        }
      >
        <AppBarCommand
          icon="play"
          label="play"
          disabled={picked.length !== 1}
          onClick={() => {
            const id = picked[0].id;
            setSelected(new Set());
            play(id);
          }}
        />
        <AppBarCommand icon="search" label="find info" disabled={picked.length !== 1} onClick={() => setDialog("match")} />
        <AppBarCommand icon="image" label="art" disabled={picked.length !== 1} onClick={() => setDialog("art")} />
        <AppBarCommand icon="grid" label="collections" onClick={() => setDialog("collections")} />
        <AppBarCommand
          icon="star"
          label={allFav ? "unfavorite" : "favorite"}
          onClick={() => picked.forEach((g) => updateGame(g.id, { favorite: !allFav }))}
        />
        <AppBarCommand icon="edit" label="rename" disabled={picked.length !== 1} onClick={() => setDialog("rename")} />
        <AppBarCommand icon="trash" label="delete" onClick={() => setDialog("delete")} />
      </AppBar>

      {dialog === "rename" && picked.length === 1 && (
        <PromptDialog
          title="rename"
          note={picked[0].originalName}
          label="Title"
          initial={picked[0].title}
          onClose={() => setDialog(null)}
          onSave={(title) => {
            updateGame(picked[0].id, { title });
            setDialog(null);
          }}
        />
      )}
      {dialog === "art" && picked.length === 1 && (
        <ArtPicker game={picked[0]} onClose={() => setDialog(null)} onFindMatch={() => setDialog("match")} />
      )}
      {dialog === "collections" && picked.length > 0 && (
        <AddToCollection gameIds={picked.map((g) => g.id)} onClose={() => setDialog(null)} />
      )}
      {dialog === "match" && picked.length === 1 && (
        <MatchDialog game={picked[0]} onClose={() => setDialog(null)} />
      )}
      {reviewing && !dialog && (
        <MatchDialog
          key={reviewing.id}
          game={reviewing}
          onClose={() => setReviewQueue((q) => q.slice(1))}
        />
      )}
      {dialog === "delete" && picked.length > 0 && (
        <ConfirmDialog
          title={picked.length === 1 ? `delete ${picked[0].title}?` : `delete ${picked.length} games?`}
          confirmLabel="delete"
          onClose={() => setDialog(null)}
          onConfirm={() => {
            deleteGames(picked.map((g) => g.id));
            setSelected(new Set());
            setDialog(null);
          }}
        >
          <p>
            This removes {picked.length === 1 ? "it" : "them"} from your library along with the launcher's copy and any art.
            Your saves are kept in case you add the same file again. Your original files aren't touched.
          </p>
        </ConfirmDialog>
      )}
    </div>
  );
}


function EmptyLibrary({ onAdd }: { onAdd: () => void }) {
  return (
    <div className={s.emptyPage}>
      <h1 className={`${s.emptyTitle} ts`} style={{ ["--i" as string]: 0 }}>
        your library's empty
      </h1>
      <p className={`${s.emptyBody} ts`} style={{ ["--i" as string]: 1 }}>
        Add your SWF files and they'll show up here. You can also drag them, or whole folders, anywhere onto this window.
      </p>
      <div className="ts" style={{ ["--i" as string]: 2 }}>
        <Button variant="accent" onClick={onAdd}>
          <Icon name="plus" size={16} /> add games
        </Button>
      </div>
    </div>
  );
}
