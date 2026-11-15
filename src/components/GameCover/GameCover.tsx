import type { ReactNode } from "react";
import type { Game } from "../../data/types";
import { formatPlayTime } from "../../lib/format";
import { useTilt } from "../../lib/useTilt";
import { FallbackArt } from "../FallbackArt/FallbackArt";
import { Icon } from "../Icon/Icon";
import s from "./GameCover.module.css";

export function GameGrid({ children }: { children: ReactNode }) {
  return <div className={s.grid}>{children}</div>;
}

/** A game's cover in a grid: art, title, play time. Click activates (opens
 * the game page, or toggles selection); right-click toggles selection. */
export function GameCover({
  game,
  index,
  selected = false,
  onActivate,
  onToggle,
}: {
  game: Game;
  index: number;
  selected?: boolean;
  onActivate: (el: HTMLElement) => void;
  onToggle?: () => void;
}) {
  const tilt = useTilt<HTMLButtonElement>();
  // The stagger animation lives on a wrapper so it never fights the tilt
  // transform on the button itself.
  return (
    <div className="stagger" style={{ ["--i" as string]: index }}>
      <button
        {...tilt}
        className={s.cover}
        data-nav
        data-selected={selected}
        aria-pressed={onToggle ? selected : undefined}
        aria-label={game.title}
        onClick={(e) => onActivate(e.currentTarget.querySelector<HTMLElement>("[data-art]") ?? e.currentTarget)}
        onContextMenu={(e) => {
          if (!onToggle) return;
          e.preventDefault();
          onToggle();
        }}
      >
        <span className={s.coverArt} data-art>
          <FallbackArt game={game} variant="cover" />
          {selected && <span className={s.check} />}
        </span>
        <span className={s.coverMeta}>
          <span className={s.coverTitle}>{game.title}</span>
          <span className={s.coverSub}>
            {game.playSeconds ? formatPlayTime(game.playSeconds) : "new"}
            {game.favorite && <Icon name="star" size={12} fill />}
          </span>
        </span>
      </button>
    </div>
  );
}
