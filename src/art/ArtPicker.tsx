import { useEffect, useMemo, useState } from "react";
import { Button, Segmented } from "../components/Controls/Controls";
import { BandDialog } from "../components/Dialog/BandDialog";
import { FallbackArt } from "../components/FallbackArt/FallbackArt";
import { ProgressDots, useToast } from "../components/Feedback/Feedback";
import { Icon } from "../components/Icon/Icon";
import { useLibrary } from "../data/LibraryProvider";
import type { ArtItem, ArtSlot, Game } from "../data/types";
import { api } from "../lib/api";
import s from "./ArtPicker.module.css";

const SLOTS: { slot: ArtSlot; label: string; hint: string }[] = [
  { slot: "cover", label: "cover", hint: "Portrait box art, used in the library grid." },
  { slot: "wide", label: "wide", hint: "Landscape art for wide tiles on the home screen." },
  { slot: "hero", label: "hero", hint: "The big banner behind a game's page and on large tiles." },
  { slot: "logo", label: "logo", hint: "A transparent logo laid over heroes and generated tiles." },
  { slot: "icon", label: "icon", hint: "A small square icon." },
  { slot: "screenshot", label: "screenshot", hint: "Gameplay. Screenshots you take while playing (F12) land here." },
  { slot: "embedded", label: "from the swf", hint: "Pictures found inside the game file." },
];

const SOURCE_LABEL: Record<string, string> = {
  swf: "from the swf",
  player: "your screenshot",
  user: "your upload",
  flashpoint: "Flashpoint",
  steamgriddb: "SteamGridDB",
  igdb: "IGDB",
  archive: "Internet Archive",
};

// Pick the image for each art slot: scraped candidates, images from the
// SWF, screenshots, or your own upload.
export function ArtPicker({ game, onClose, onFindMatch }: { game: Game; onClose: () => void; onFindMatch?: () => void }) {
  const { reloadGame, games } = useLibrary();
  const toast = useToast();
  const [items, setItems] = useState<ArtItem[] | null>(null);
  const [slot, setSlot] = useState<ArtSlot>("cover");
  const [busy, setBusy] = useState<number | "upload" | "refresh" | null>(null);
  const live = games.find((g) => g.id === game.id) ?? game;

  const load = () =>
    api
      .listArt(game.id)
      .then(setItems)
      .catch((e) => toast({ title: "couldn't load art", body: String(e) }));

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [game.id]);

  const counts = useMemo(() => {
    const c: Partial<Record<ArtSlot, number>> = {};
    for (const a of items ?? []) c[a.slot] = (c[a.slot] ?? 0) + 1;
    return c;
  }, [items]);

  const inSlot = (items ?? []).filter((a) => a.slot === slot);
  const noneSelected = !inSlot.some((a) => a.selected);
  const info = SLOTS.find((x) => x.slot === slot)!;

  const run = async (key: number | "upload" | "refresh", fn: () => Promise<unknown>) => {
    setBusy(key);
    try {
      await fn();
      await reloadGame(game.id);
      await load();
    } catch (e) {
      toast({ title: "that didn't work", body: String(e) });
    } finally {
      setBusy(null);
    }
  };

  const choose = (a: ArtItem | null) =>
    run(a?.id ?? -1, () => api.selectArt(game.id, a?.id ?? null, slot));

  const upload = async () => {
    const path = await api.pickImage();
    if (path) run("upload", () => api.uploadArt(game.id, slot, path));
  };

  return (
    <BandDialog
      wide
      tall
      title={`art for ${live.title}`}
      onCancel={onClose}
      actions={
        <>
          {onFindMatch && (
            <Button onClick={onFindMatch}>
              <Icon name="search" size={16} /> find a match
            </Button>
          )}
          <Button onClick={() => run("refresh", () => api.scrapeRefresh(game.id))} disabled={busy !== null || !live.links.length}>
            look up again
          </Button>
          <Button variant="accent" onClick={onClose} data-autofocus>
            done
          </Button>
        </>
      }
    >
      <div className={s.top}>
        <Segmented<ArtSlot>
          label="Art slot"
          value={slot}
          onChange={setSlot}
          options={SLOTS.map((x) => ({
            value: x.slot,
            label: (
              <>
                {x.label}
                {counts[x.slot] ? <span className={s.count}>{counts[x.slot]}</span> : null}
              </>
            ),
          }))}
        />
      </div>
      <div className={s.hintRow}>
        <p className={s.lead}>{info.hint}</p>
        {slot !== "embedded" && (
          <Button onClick={upload} disabled={busy !== null}>
            <Icon name="upload" size={16} /> use your own
          </Button>
        )}
      </div>
      {busy === "refresh" || busy === "upload" ? <ProgressDots /> : null}
      {items === null ? (
        <ProgressDots />
      ) : (
        <div className={s.grid} data-slot={slot}>
          <Option selected={noneSelected} label="none" onClick={() => choose(null)} slot={slot}>
            <div className={s.noneBox}>
              {slot === "cover" ? (
                <div className={s.coverPreview}>
                  <FallbackArt game={{ ...live, art: { ...live.art, cover: null } }} variant="cover" />
                </div>
              ) : (
                <span>none</span>
              )}
            </div>
          </Option>
          {inSlot.map((a) => (
            <Option
              key={a.id}
              slot={slot}
              selected={a.selected}
              busy={busy === a.id}
              label={SOURCE_LABEL[a.source] ?? a.source}
              meta={a.width && a.height ? `${a.width}×${a.height}` : undefined}
              onClick={() => choose(a)}
              onDelete={a.source === "player" || a.source === "user" ? () => run(a.id, () => api.deleteArt(game.id, a.id)) : undefined}
            >
              <img
                src={a.path ? api.fileUrl(a.path) : (a.thumbUrl ?? a.remoteUrl ?? "")}
                alt=""
                draggable={false}
                loading="lazy"
              />
            </Option>
          ))}
        </div>
      )}
      {items !== null && inSlot.length === 0 && (
        <p className={s.empty}>
          {slot === "embedded"
            ? "No pictures were found inside this SWF."
            : live.links.length
              ? "None of the matched sources have this kind of art. Add a SteamGridDB key in settings for covers, heroes and logos."
              : "Nothing yet. Find a match first, or use your own image."}
        </p>
      )}
    </BandDialog>
  );
}

function Option({
  selected,
  label,
  meta,
  onClick,
  onDelete,
  busy,
  slot,
  children,
}: {
  selected: boolean;
  label: string;
  meta?: string;
  onClick: () => void;
  onDelete?: () => void;
  busy?: boolean;
  slot: ArtSlot;
  children: React.ReactNode;
}) {
  return (
    <div className={s.option}>
      <button className={s.pick} data-slot={slot} data-selected={selected} aria-pressed={selected} onClick={onClick} data-nav>
        {children}
        {selected && <span className={s.check} />}
        {busy && (
          <span className={s.busy}>
            <ProgressDots />
          </span>
        )}
      </button>
      <div className={s.caption}>
        <span>
          {label}
          {meta && <span className={s.meta}> · {meta}</span>}
        </span>
        {onDelete && (
          <button className={s.delete} onClick={onDelete} aria-label="Delete image" title="Delete image">
            <Icon name="trash" size={14} />
          </button>
        )}
      </div>
    </div>
  );
}
