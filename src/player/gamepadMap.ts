// Controller -> keyboard layouts for games. Ruffle reads the controller
// itself and presses the mapped keys (its gamepadButtonMapping option), so
// a layout is just button name -> Flash key code.

import type { Game } from "../data/types";

export type PadButton =
  | "dpad-up"
  | "dpad-down"
  | "dpad-left"
  | "dpad-right"
  | "south"
  | "east"
  | "west"
  | "north"
  | "left-trigger"
  | "right-trigger"
  | "left-trigger-2"
  | "right-trigger-2"
  | "select"
  | "start";

export type PadMap = Partial<Record<PadButton, number>>;

/** In the order the controller dialog lists them, with Xbox-style names. */
export const PAD_BUTTONS: { id: PadButton; label: string }[] = [
  { id: "dpad-up", label: "d-pad up" },
  { id: "dpad-down", label: "d-pad down" },
  { id: "dpad-left", label: "d-pad left" },
  { id: "dpad-right", label: "d-pad right" },
  { id: "south", label: "A" },
  { id: "east", label: "B" },
  { id: "west", label: "X" },
  { id: "north", label: "Y" },
  { id: "left-trigger", label: "LB" },
  { id: "right-trigger", label: "RB" },
  { id: "left-trigger-2", label: "LT" },
  { id: "right-trigger-2", label: "RT" },
  { id: "select", label: "view / back" },
  { id: "start", label: "menu / start" },
];

const K = { left: 37, up: 38, right: 39, down: 40, space: 32, enter: 13, shift: 16, ctrl: 17, esc: 27 };
const ch = (c: string) => c.toUpperCase().charCodeAt(0);

export const PRESETS: { id: string; label: string; map: PadMap }[] = [
  {
    id: "arrows",
    label: "arrows + space",
    map: {
      "dpad-up": K.up,
      "dpad-down": K.down,
      "dpad-left": K.left,
      "dpad-right": K.right,
      south: K.space,
      east: ch("x"),
      west: ch("z"),
      north: ch("c"),
      "left-trigger": K.shift,
      "right-trigger": K.ctrl,
      start: K.enter,
      select: ch("p"),
    },
  },
  {
    id: "wasd",
    label: "wasd",
    map: {
      "dpad-up": ch("w"),
      "dpad-down": ch("s"),
      "dpad-left": ch("a"),
      "dpad-right": ch("d"),
      south: K.space,
      east: ch("e"),
      west: ch("q"),
      north: ch("r"),
      "left-trigger": K.shift,
      "right-trigger": ch("f"),
      start: K.enter,
      select: ch("p"),
    },
  },
];

export const DEFAULT_PRESET = PRESETS[0];

export function padMapFor(game: Game): PadMap {
  if (!game.gamepadMap) return DEFAULT_PRESET.map;
  try {
    const m = JSON.parse(game.gamepadMap);
    return m && typeof m === "object" ? (m as PadMap) : DEFAULT_PRESET.map;
  } catch {
    return DEFAULT_PRESET.map;
  }
}

/** Which preset a layout matches, or "custom". */
export function presetOf(map: PadMap): string {
  const same = (a: PadMap, b: PadMap) => {
    const ka = Object.keys(a).filter((k) => a[k as PadButton] != null);
    const kb = Object.keys(b).filter((k) => b[k as PadButton] != null);
    return ka.length === kb.length && ka.every((k) => a[k as PadButton] === b[k as PadButton]);
  };
  return PRESETS.find((p) => same(p.map, map))?.id ?? "custom";
}

const NAMES: Record<number, string> = {
  8: "backspace",
  9: "tab",
  13: "enter",
  16: "shift",
  17: "ctrl",
  18: "alt",
  27: "esc",
  32: "space",
  33: "page up",
  34: "page down",
  35: "end",
  36: "home",
  37: "←",
  38: "↑",
  39: "→",
  40: "↓",
  45: "insert",
  46: "delete",
  186: ";",
  187: "=",
  188: ",",
  189: "-",
  190: ".",
  191: "/",
  192: "`",
  219: "[",
  220: "\\",
  221: "]",
  222: "'",
};

export function keyName(code: number | undefined): string {
  if (code == null) return "—";
  if (NAMES[code]) return NAMES[code];
  if ((code >= 48 && code <= 57) || (code >= 65 && code <= 90)) return String.fromCharCode(code);
  if (code >= 96 && code <= 105) return `num ${code - 96}`;
  if (code >= 112 && code <= 123) return `F${code - 111}`;
  return `key ${code}`;
}

/** Flash key code for a browser key event (Flash uses the classic codes). */
export function codeFromEvent(e: KeyboardEvent): number | null {
  // keyCode is deprecated but it's exactly what Flash expects.
  const c = e.keyCode || e.which;
  return c > 0 && c < 256 ? c : null;
}
