import { useEffect, useState } from "react";
import { Button, TextField } from "../components/Controls/Controls";
import { BandDialog } from "../components/Dialog/BandDialog";
import { ProgressDots, useToast } from "../components/Feedback/Feedback";
import { Icon } from "../components/Icon/Icon";
import { useLibrary } from "../data/LibraryProvider";
import type { Game, ScrapeMatch, SearchGroup } from "../data/types";
import { api } from "../lib/api";
import s from "./MatchDialog.module.css";

/** "none" means: don't use this source for this game. */
type Choice = string | "none";

// Search every source for a game and pick the right entry in each. The
// best hit is preselected when it's a clear match; the user confirms.
export function MatchDialog({ game, onClose, onDone }: { game: Game; onClose: () => void; onDone?: () => void }) {
  const { reloadGame } = useLibrary();
  const toast = useToast();
  const [query, setQuery] = useState(game.title);
  const [groups, setGroups] = useState<SearchGroup[] | null>(null);
  const [choice, setChoice] = useState<Record<string, Choice>>({});
  const [searching, setSearching] = useState(false);
  const [applying, setApplying] = useState(false);

  const search = async (q: string) => {
    setSearching(true);
    try {
      const res = await api.scrapeSearch(game.id, q);
      setGroups(res);
      const pre: Record<string, Choice> = {};
      for (const g of res) {
        const linked = g.linkedId && g.matches.find((m) => m.id === g.linkedId);
        const best = g.matches[0];
        const clear = best && (best.fileMatch || (best.score >= 0.92 && (g.matches[1]?.score ?? 0) <= best.score - 0.05));
        pre[g.source] = linked ? linked.id : clear ? best.id : "none";
      }
      setChoice(pre);
    } catch (e) {
      toast({ title: "search failed", body: String(e) });
    } finally {
      setSearching(false);
    }
  };

  useEffect(() => {
    search(game.title);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id]);

  const picks: ScrapeMatch[] = (groups ?? []).flatMap((g) => {
    const c = choice[g.source];
    const m = c && c !== "none" ? g.matches.find((x) => x.id === c) : undefined;
    return m ? [m] : [];
  });

  const apply = async () => {
    setApplying(true);
    try {
      // Sources switched to "none" that were linked before get unlinked.
      for (const g of groups ?? []) {
        if (g.linkedId && choice[g.source] === "none") await api.scrapeUnlink(game.id, g.source);
      }
      const report = picks.length ? await api.scrapeApply(game.id, picks) : null;
      await reloadGame(game.id);
      if (report?.errors.length) {
        toast({ title: "some art couldn't be fetched", body: report.errors.slice(0, 2).join(" · ") });
      } else if (report) {
        toast({ title: `matched ${report.title}`, body: "Info and art are updated." });
      }
      onDone?.();
      onClose();
    } catch (e) {
      toast({ title: "couldn't use that match", body: String(e) });
    } finally {
      setApplying(false);
    }
  };

  return (
    <BandDialog
      wide
      tall
      title={`find ${game.title}`}
      onCancel={applying ? undefined : onClose}
      actions={
        <>
          <Button variant="accent" onClick={apply} disabled={applying || searching || !groups}>
            {applying ? "fetching art…" : picks.length ? "use these" : "save"}
          </Button>
          <Button onClick={onClose} disabled={applying}>
            cancel
          </Button>
        </>
      }
    >
      <div className={s.searchRow}>
        <TextField label="Search for" value={query} onChange={setQuery} onEnter={() => search(query)} />
        <Button onClick={() => search(query)} disabled={searching}>
          <Icon name="search" size={16} /> search
        </Button>
      </div>
      {searching || applying ? (
        <div className={s.dots}>
          <ProgressDots label={applying ? "Fetching art" : "Searching"} />
        </div>
      ) : null}
      <div className={s.groups}>
        {(groups ?? []).map((g) => (
          <section key={g.source} className={s.group}>
            <h3 className={s.groupTitle}>
              {g.label}
              {g.unavailable && <span className={s.note}>{g.unavailable}</span>}
              {g.error && <span className={s.error}>{g.error}</span>}
            </h3>
            {!g.unavailable && !g.error && (
              <div className={s.matches} role="radiogroup" aria-label={g.label}>
                {g.matches.map((m) => (
                  <MatchCard
                    key={m.id}
                    m={m}
                    selected={choice[g.source] === m.id}
                    onPick={() => setChoice((c) => ({ ...c, [g.source]: m.id }))}
                  />
                ))}
                <button
                  className={s.none}
                  role="radio"
                  aria-checked={choice[g.source] === "none"}
                  data-selected={choice[g.source] === "none"}
                  onClick={() => setChoice((c) => ({ ...c, [g.source]: "none" }))}
                  data-nav
                >
                  {g.matches.length ? "none of these" : "nothing found"}
                </button>
              </div>
            )}
          </section>
        ))}
      </div>
    </BandDialog>
  );
}

function MatchCard({ m, selected, onPick }: { m: ScrapeMatch; selected: boolean; onPick: () => void }) {
  const pct = Math.round(m.score * 100);
  return (
    <button className={s.card} role="radio" aria-checked={selected} data-selected={selected} onClick={onPick} data-nav>
      <span className={s.thumb}>{m.thumb ? <img src={m.thumb} alt="" loading="lazy" /> : <Icon name="image" size={22} />}</span>
      <span className={s.text}>
        <span className={s.title}>{m.title}</span>
        <span className={s.sub}>{[m.developer, m.year, m.platform].filter(Boolean).join(" · ") || " "}</span>
        <span className={s.badges}>
          <span className={s.score} data-high={pct >= 92}>
            {pct}% match
          </span>
          {m.fileMatch && <span className={s.file}>same file name</span>}
        </span>
      </span>
      {selected && <span className={s.check} />}
    </button>
  );
}
