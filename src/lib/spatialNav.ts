// Arrow-key (and later, gamepad d-pad) navigation between anything marked
// with data-nav. Picks the nearest element in the pressed direction, so tile
// grids of mixed sizes work without hand-written neighbor maps.

type Dir = "left" | "right" | "up" | "down";

const KEYS: Record<string, Dir> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
};

function visible(el: HTMLElement): boolean {
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 && !el.closest("[inert],[aria-hidden='true']");
}

export function moveFocus(dir: Dir, from: HTMLElement | null): boolean {
  const all = Array.from(document.querySelectorAll<HTMLElement>("[data-nav]")).filter(visible);
  if (!all.length) return false;

  if (!from || !from.matches("[data-nav]")) {
    all[0].focus();
    return true;
  }

  const a = from.getBoundingClientRect();
  const ax = a.left + a.width / 2;
  const ay = a.top + a.height / 2;

  let best: HTMLElement | null = null;
  let bestScore = Infinity;

  for (const el of all) {
    if (el === from) continue;
    const b = el.getBoundingClientRect();
    const bx = b.left + b.width / 2;
    const by = b.top + b.height / 2;
    const dx = bx - ax;
    const dy = by - ay;

    let primary: number;
    let cross: number;
    switch (dir) {
      case "right":
        if (b.left < a.right - 4) continue;
        primary = b.left - a.right;
        cross = Math.abs(dy);
        break;
      case "left":
        if (b.right > a.left + 4) continue;
        primary = a.left - b.right;
        cross = Math.abs(dy);
        break;
      case "down":
        if (b.top < a.bottom - 4) continue;
        primary = b.top - a.bottom;
        cross = Math.abs(dx);
        break;
      case "up":
        if (b.bottom > a.top + 4) continue;
        primary = a.top - b.bottom;
        cross = Math.abs(dx);
        break;
    }
    // Strongly prefer staying in the same row/column.
    const score = primary + cross * 2.5;
    if (score < bestScore) {
      bestScore = score;
      best = el;
    }
  }

  if (!best) return false;
  best.focus({ preventScroll: true });
  best.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
  return true;
}

let enabled = true;

/** Turned off while a game has the keyboard, so arrows reach the game. */
export function setSpatialNavEnabled(on: boolean) {
  enabled = on;
}

export function isSpatialNavEnabled(): boolean {
  return enabled;
}

export function installSpatialNav(): () => void {
  const onKey = (e: KeyboardEvent) => {
    if (!enabled) return;
    const dir = KEYS[e.key];
    if (!dir || e.altKey || e.ctrlKey || e.metaKey) return;
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT")) return;
    if (moveFocus(dir, t)) e.preventDefault();
  };
  window.addEventListener("keydown", onKey);
  return () => window.removeEventListener("keydown", onKey);
}
