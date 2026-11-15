import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { useTilt } from "../../lib/useTilt";
import { useReducedMotion } from "../../theme/ThemeProvider";
import s from "./Tile.module.css";

export type TileSize = "small" | "medium" | "wide" | "large";

const SPAN: Record<TileSize, [number, number]> = {
  small: [1, 1],
  medium: [2, 2],
  wide: [4, 2],
  large: [4, 4],
};

interface TileProps {
  size: TileSize;
  /** One face is a static tile. Two or more make it a live tile. */
  faces: ReactNode[];
  label?: string;
  badge?: string | number;
  color?: string;
  /** Average seconds between live flips. Each tile adds jitter so they never sync up. */
  interval?: number;
  /** Gets the tile element, so pages can grow out of it. */
  onClick?: (el: HTMLElement) => void;
  ariaLabel?: string;
  style?: CSSProperties;
}

export function Tile({ size, faces, label, badge, color, interval = 7, onClick, ariaLabel, style }: TileProps) {
  const [w, h] = SPAN[size];
  // flips counts how many times the tile has turned over; the current face
  // is flips % faces.length. Keying faces by flip count remounts them so
  // each flip replays the slide-in / slide-out keyframes.
  const [flips, setFlips] = useState(0);
  const reduced = useReducedMotion();
  const tilt = useTilt<HTMLButtonElement>();
  const live = faces.length > 1 && !reduced;

  useEffect(() => {
    if (!live) return;
    let t: number;
    const schedule = (first: boolean) => {
      const base = interval * 1000;
      const jitter = Math.random() * base * 0.8;
      t = window.setTimeout(
        () => {
          if (!document.hidden) {
            setFlips((n) => n + 1);
          }
          schedule(false);
        },
        first ? 1500 + jitter * 2 : base + jitter,
      );
    };
    schedule(true);
    return () => window.clearTimeout(t);
  }, [live, interval, faces.length]);

  return (
    <button
      {...tilt}
      className={s.tile}
      data-size={size}
      data-nav
      onClick={(e) => onClick?.(e.currentTarget)}
      aria-label={ariaLabel ?? label}
      style={{ gridColumn: `span ${w}`, gridRow: `span ${h}`, background: color, ...style }}
    >
      <span className={s.faces}>
        {flips > 0 && (
          <span key={`out-${flips}`} className={s.face} data-state="out">
            {faces[(flips - 1) % faces.length]}
          </span>
        )}
        <span key={`in-${flips}`} className={s.face} data-state={flips > 0 ? "in" : "still"}>
          {faces[flips % faces.length]}
        </span>
      </span>
      {label && size !== "small" && <span className={s.label}>{label}</span>}
      {badge !== undefined && <span className={s.badge}>{badge}</span>}
    </button>
  );
}
