// Thin line glyphs in the spirit of Segoe UI Symbol / MDL2. Square caps,
// mitered joins, nothing rounded.
const PATHS: Record<string, string> = {
  plus: "M12 4v16M4 12h16",
  gear: "M12 8.5a3.5 3.5 0 1 0 0 7a3.5 3.5 0 1 0 0-7zM12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1",
  minimize: "M5 12h14",
  maximize: "M5 5h14v14H5z",
  restore: "M8 8h11v11H8zM5 16V5h11",
  close: "M5 5l14 14M19 5L5 19",
  grid: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  list: "M4 6h16M4 12h16M4 18h16",
  search: "M10.5 4a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13zM15.5 15.5L21 21",
  chevron: "M9 5l7 7l-7 7",
  play: "M7 4l13 8l-13 8z",
  bolt: "M14 2L5 13h6l-2 9l9-12h-6l2-8z",
  upload: "M12 16V4M6 10l6-6l6 6M4 20h16",
  folder: "M3 6h6l2 2h10v11H3z",
  back: "M20 12H5M11 5l-7 7l7 7",
  image: "M3 5h18v14H3zM3 16l5-5l4 4l3-3l6 6M15.5 8.5a1 1 0 1 0 0 .1",
  trash: "M4 6h16M9 6V3h6v3M6 6l1 15h10l1-15M10 10v7M14 10v7",
  edit: "M4 20h4L19 9l-4-4L4 16zM13 7l4 4",
  clear: "M4 4l16 16M4 20L20 4",
  download: "M12 4v12M6 10l6 6l6-6M4 20h16",
  archive: "M3 4h18v5H3zM5 9v11h14V9M10 13h4",
  gamepad: "M7 8h10l4 9l-3 2l-4-4h-4l-4 4l-3-2zM8 11v3M6.5 12.5h3M15.5 12h.1M17.5 14h.1",
  star: "M12 3l2.7 5.8l6.3.7l-4.7 4.3l1.3 6.2L12 16.9L6.4 20l1.3-6.2L3 9.5l6.3-.7z",
};

export type IconName = keyof typeof PATHS;

export function Icon({ name, size = 20, fill = false }: { name: IconName; size?: number; fill?: boolean }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill={fill ? "currentColor" : "none"}
      stroke="currentColor"
      strokeWidth={fill ? 0 : 1.5}
      strokeLinecap="square"
      strokeLinejoin="miter"
      aria-hidden="true"
    >
      <path d={PATHS[name]} />
    </svg>
  );
}
