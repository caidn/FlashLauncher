import { useEffect, useMemo, useState } from "react";
import { AppBar, AppBarCommand } from "../../components/AppBar/AppBar";
import { Button, Checkbox } from "../../components/Controls/Controls";
import { BandDialog } from "../../components/Dialog/BandDialog";
import { ConfirmDialog, PromptDialog } from "../../components/Dialog/Dialogs";
import { GameCover, GameGrid } from "../../components/GameCover/GameCover";
import { Icon } from "../../components/Icon/Icon";
import { useLibrary } from "../../data/LibraryProvider";
import type { Collection, Game } from "../../data/types";
import { useNav } from "../../nav";
import s from "./Collections.module.css";

// One collection: its games in the order they were added. Right-click
// games to select them, then move or remove them from the bar below.
export function CollectionView({ collection }: { collection: Collection }) {
  const { games, renameCollection, deleteCollection, removeFromCollection, addToCollection } = useLibrary();
  const { openGame, back } = useNav();
  const [dialog, setDialog] = useState<"rename" | "delete" | "add" | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const byId = useMemo(() => new Map(games.map((g) => [g.id, g])), [games]);
  const members = collection.gameIds.map((id) => byId.get(id)).filter((g): g is Game => !!g);

  const toggle = (id: number) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (!next.delete(id)) next.add(id);
      return next;
    });

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

  return (
    <div className={s.view}>
      <header className={`${s.viewHead} ts`} style={{ ["--i" as string]: 0 }}>
        <button className={s.back} onClick={back} aria-label="Back" data-nav>
          <Icon name="back" size={20} />
        </button>
        <div className={s.viewTitleBox}>
          <span className={s.kicker}>collection</span>
          <h1 className={s.viewTitle}>{collection.name}</h1>
        </div>
        <div className={s.viewActions}>
          <Button variant="accent" onClick={() => setDialog("add")}>
            <Icon name="plus" size={16} /> add games
          </Button>
          <Button onClick={() => setDialog("rename")}>rename</Button>
          <Button onClick={() => setDialog("delete")}>delete</Button>
        </div>
      </header>

      <div className={s.viewCount}>
        {members.length} {members.length === 1 ? "game" : "games"}
        {selected.size > 0 && ` · ${selected.size} selected`}
      </div>

      <div className={s.viewScroll} data-bar={selected.size > 0}>
        {members.length === 0 ? (
          <p className={s.none}>nothing in here yet</p>
        ) : (
          <GameGrid>
            {members.map((g, i) => (
              <GameCover
                key={g.id}
                game={g}
                index={i}
                selected={selected.has(g.id)}
                onActivate={(el) => (selected.size ? toggle(g.id) : openGame(g.id, el))}
                onToggle={() => toggle(g.id)}
              />
            ))}
          </GameGrid>
        )}
      </div>

      <AppBar
        open={selected.size > 0}
        left={
          <>
            <span>{selected.size === 1 ? byId.get([...selected][0])?.title : `${selected.size} games`}</span>
            <Button onClick={() => setSelected(new Set())}>clear</Button>
          </>
        }
      >
        <AppBarCommand
          icon="close"
          label="remove"
          onClick={() => {
            removeFromCollection(collection.id, [...selected]);
            setSelected(new Set());
          }}
        />
      </AppBar>

      {dialog === "rename" && (
        <PromptDialog
          title="rename collection"
          label="Collection name"
          initial={collection.name}
          onClose={() => setDialog(null)}
          onSave={(name) => {
            renameCollection(collection.id, name);
            setDialog(null);
          }}
        />
      )}
      {dialog === "delete" && (
        <ConfirmDialog
          title={`delete ${collection.name}?`}
          confirmLabel="delete"
          onClose={() => setDialog(null)}
          onConfirm={() => {
            setDialog(null);
            deleteCollection(collection.id);
          }}
        >
          <p>Only the collection goes. The games stay in your library.</p>
        </ConfirmDialog>
      )}
      {dialog === "add" && (
        <AddGamesDialog
          collection={collection}
          games={games}
          onClose={() => setDialog(null)}
          onAdd={(ids) => {
            addToCollection(collection.id, ids);
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}

function AddGamesDialog({
  collection,
  games,
  onClose,
  onAdd,
}: {
  collection: Collection;
  games: Game[];
  onClose: () => void;
  onAdd: (ids: number[]) => void;
}) {
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const [q, setQ] = useState("");
  const candidates = games
    .filter((g) => !collection.gameIds.includes(g.id))
    .filter((g) => !q.trim() || g.title.toLowerCase().includes(q.trim().toLowerCase()))
    .sort((a, b) => a.title.localeCompare(b.title));

  return (
    <BandDialog
      wide
      tall
      title={`add to ${collection.name}`}
      onCancel={onClose}
      actions={
        <>
          <Button variant="accent" disabled={!picked.size} onClick={() => onAdd([...picked])}>
            {picked.size ? `add ${picked.size}` : "add"}
          </Button>
          <Button onClick={onClose}>cancel</Button>
        </>
      }
    >
      <label className={s.filter}>
        <Icon name="search" size={16} />
        <input placeholder="filter games" value={q} onChange={(e) => setQ(e.target.value)} data-autofocus />
      </label>
      <ul className={s.pickList}>
        {candidates.map((g) => (
          <li key={g.id}>
            <Checkbox
              label={g.title}
              checked={picked.has(g.id)}
              onChange={(v) =>
                setPicked((prev) => {
                  const next = new Set(prev);
                  if (v) next.add(g.id);
                  else next.delete(g.id);
                  return next;
                })
              }
            />
            <span className={s.swatch} style={{ background: g.color }} />
            <span className={s.pickTitle}>{g.title}</span>
            <span className={s.pickSub}>{[g.developer, g.year].filter(Boolean).join(" · ")}</span>
          </li>
        ))}
        {candidates.length === 0 && <li className={s.pickEmpty}>every game is already in here</li>}
      </ul>
    </BandDialog>
  );
}
