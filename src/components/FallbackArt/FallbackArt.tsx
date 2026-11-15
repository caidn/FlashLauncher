import type { Game } from "../../data/types";
import { api } from "../../lib/api";
import { readableOn } from "../../theme/accents";
import { Icon } from "../Icon/Icon";
import s from "./FallbackArt.module.css";

/** cover: portrait 2:3 · square: medium/large tiles · tile: wide tiles · hero: banners */
type Variant = "cover" | "square" | "tile" | "hero";

interface Picked {
  src: string;
  /** "full" fills the box (a real cover, hero, screenshot); "inset" sits
   * uncropped inside a generated tile (logos, sprites, title cards). */
  mode: "full" | "inset";
  /** True for art that already shows the game's name (real covers). */
  named: boolean;
}

/** Chooses what to show for a game in a given shape, best art first. */
export function pickArt(game: Game, variant: Variant): Picked | null {
  const a = game.art ?? {};
  const full = (src: string | null | undefined, named = false): Picked | null =>
    src ? { src, mode: "full", named } : null;
  const inset = (src: string | null | undefined): Picked | null => (src ? { src, mode: "inset", named: false } : null);
  const fallback = inset(a.logo) ?? inset(a.embedded) ?? full(a.screenshot);

  if (variant === "cover") {
    // No real cover: a logo or a title card (Flashpoint's "logos" are
    // landscape title cards) inside the generated tile beats a cropped
    // screenshot, because it shows the game's actual name art.
    return full(a.cover, true) ?? inset(a.logo) ?? inset(a.wide) ?? inset(a.embedded) ?? full(a.screenshot);
  }
  if (variant === "square") {
    // Landscape title cards cropped to a square lose the name, so they sit
    // inside the tile instead; real covers, heroes and screenshots crop fine.
    return full(a.cover, true) ?? full(a.hero) ?? inset(a.logo) ?? inset(a.wide) ?? full(a.screenshot) ?? inset(a.embedded);
  }
  // wide tiles and heroes are landscape
  return full(a.hero) ?? full(a.wide, true) ?? full(a.screenshot) ?? (variant === "tile" ? full(a.cover, true) : null) ?? fallback;
}

// Art for a game: real scraped art when there is some, otherwise a flat
// Metro tile in the game's color with the title set big, plus whatever
// picture we have (logo, image from inside the SWF, screenshot).
export function FallbackArt({ game, variant, compact = false }: { game: Game; variant: Variant; compact?: boolean }) {
  const pick = pickArt(game, variant);
  const logo = game.art?.logo;
  const fg = readableOn(game.color);

  // Real art that fills the box. Heroes and screenshots get the logo (or a
  // title band) on top, Steam-library style; real covers already say it.
  if (pick?.mode === "full") {
    const overlayLogo = !pick.named && logo && variant !== "cover";
    return (
      <div className={s.art} data-variant={variant} data-full="true" data-compact={compact} style={{ background: game.color, color: fg }} aria-hidden="true">
        <span className={s.image}>
          <img src={api.fileUrl(pick.src)} alt="" draggable={false} />
        </span>
        {overlayLogo ? (
          <span className={s.logoOver}>
            <img src={api.fileUrl(logo)} alt="" draggable={false} />
          </span>
        ) : (
          !pick.named && <span className={s.title}>{game.title}</span>
        )}
      </div>
    );
  }

  return (
    <div
      className={s.art}
      data-variant={variant}
      data-has-image={!!pick}
      data-logo={pick?.src === logo}
      data-compact={compact}
      style={{ background: game.color, color: fg }}
      aria-hidden="true"
    >
      {pick ? (
        <span className={s.image}>
          <img src={api.fileUrl(pick.src)} alt="" draggable={false} />
        </span>
      ) : (
        <span className={s.glyph}>
          <Icon name="bolt" size={variant === "hero" ? 160 : 72} fill />
        </span>
      )}
      <span className={s.meta}>swf · as{game.asVersion}</span>
      {pick?.src !== logo && <span className={s.title}>{game.title}</span>}
    </div>
  );
}
