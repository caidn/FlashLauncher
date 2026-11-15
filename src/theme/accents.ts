// The classic Windows Phone / Windows 8 accent palette.
export interface Accent {
  id: string;
  name: string;
  hex: string;
}

export const ACCENTS: Accent[] = [
  { id: "lime", name: "lime", hex: "#a4c400" },
  { id: "green", name: "green", hex: "#60a917" },
  { id: "emerald", name: "emerald", hex: "#008a00" },
  { id: "teal", name: "teal", hex: "#00aba9" },
  { id: "cyan", name: "cyan", hex: "#1ba1e2" },
  { id: "cobalt", name: "cobalt", hex: "#0050ef" },
  { id: "indigo", name: "indigo", hex: "#6a00ff" },
  { id: "violet", name: "violet", hex: "#aa00ff" },
  { id: "pink", name: "pink", hex: "#f472d0" },
  { id: "magenta", name: "magenta", hex: "#d80073" },
  { id: "crimson", name: "crimson", hex: "#a20025" },
  { id: "red", name: "red", hex: "#e51400" },
  { id: "orange", name: "orange", hex: "#fa6800" },
  { id: "amber", name: "amber", hex: "#f0a30a" },
  { id: "yellow", name: "yellow", hex: "#e3c800" },
  { id: "brown", name: "brown", hex: "#825a2c" },
  { id: "olive", name: "olive", hex: "#6d8764" },
  { id: "steel", name: "steel", hex: "#647687" },
  { id: "mauve", name: "mauve", hex: "#76608a" },
  { id: "taupe", name: "taupe", hex: "#87794e" },
];

export const DEFAULT_ACCENT = "cyan";

export function accentById(id: string): Accent {
  return ACCENTS.find((a) => a.id === id) ?? ACCENTS.find((a) => a.id === DEFAULT_ACCENT)!;
}

/** Bright accents (yellow, lime, amber) need dark text to stay readable. */
export function readableOn(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const r = (n >> 16) & 255;
  const g = (n >> 8) & 255;
  const b = n & 255;
  const lum = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
  return lum > 0.62 ? "#111111" : "#ffffff";
}
