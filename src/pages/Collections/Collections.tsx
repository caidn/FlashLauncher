import { useState } from "react";
import { Button } from "../../components/Controls/Controls";
import { PromptDialog } from "../../components/Dialog/Dialogs";
import { FallbackArt } from "../../components/FallbackArt/FallbackArt";
import { Icon } from "../../components/Icon/Icon";
import { useLibrary } from "../../data/LibraryProvider";
import type { Collection, Game } from "../../data/types";
import { useTilt } from "../../lib/useTilt";
import { useNav } from "../../nav";
import p from "../page.module.css";
import s from "./Collections.module.css";

export function Collections() {
  const { collections, games, createCollection } = useLibrary();
  const { openCollection } = useNav();
  const [creating, setCreating] = useState(false);
  const byId = new Map(games.map((g) => [g.id, g]));

  return (
    <div className={p.page}>
      <div className={`${s.head} ts`} style={{ ["--i" as string]: 0 }}>
        <p className={p.body}>
          Group games however you like. A game can be in as many collections as you want. Add games from the library
          (right-click, collections) or from a game's page.
        </p>
        <Button variant="accent" onClick={() => setCreating(true)}>
          <Icon name="plus" size={16} /> new collection
        </Button>
      </div>
      {collections.length === 0 && (
        <p className={`${s.none} ts`} style={{ ["--i" as string]: 1 }}>
          no collections yet
        </p>
      )}
      <div className={s.grid}>
        {collections.map((c, i) => (
          <CollectionCard
            key={c.id}
            c={c}
            games={c.gameIds.map((id) => byId.get(id)).filter((g): g is Game => !!g)}
            index={i + 1}
            onOpen={() => openCollection(c.id)}
          />
        ))}
      </div>
      {creating && (
        <PromptDialog
          title="new collection"
          label="Collection name"
          confirmLabel="create"
          onClose={() => setCreating(false)}
          onSave={async (name) => {
            setCreating(false);
            const c = await createCollection(name);
            if (c) openCollection(c.id);
          }}
        />
      )}
    </div>
  );
}

function CollectionCard({ c, games, index, onOpen }: { c: Collection; games: Game[]; index: number; onOpen: () => void }) {
  const tilt = useTilt<HTMLButtonElement>();
  return (
    <div className="ts" style={{ ["--i" as string]: index }}>
      <button {...tilt} className={s.card} onClick={onOpen} data-nav>
        <span className={s.strip}>
          {games.slice(0, 4).map((g) => (
            <span key={g.id} className={s.cell}>
              <FallbackArt game={g} variant="cover" compact />
            </span>
          ))}
          {Array.from({ length: Math.max(0, 4 - games.length) }, (_, i) => (
            <span key={`e${i}`} className={s.emptyCell} />
          ))}
        </span>
        <span className={s.name}>{c.name}</span>
        <span className={s.sub}>
          {games.length} {games.length === 1 ? "game" : "games"}
        </span>
      </button>
    </div>
  );
}
