// Fake library used only when the UI runs in a plain browser (npm run dev),
// where there's no Rust backend. Every title here is made up.
import type { Collection, Game } from "./types";

const day = 86_400_000;
const ago = (d: number) => new Date(Date.now() - d * day).toISOString();

type Seed = [title: string, dev: string, year: number, as: 1 | 2 | 3, compat: Game["compat"], color: string, minutes: number, lastDays: number | null, addedDays: number, fav: boolean, tags: string[]];

const SEEDS: Seed[] = [
  ["Paper Plane Derby", "Kitebox", 2008, 2, "works", "#1ba1e2", 754, 0.1, 40, true, ["arcade", "flying"]],
  ["Gravity Goo", "Spoonworks", 2010, 3, "partial", "#d80073", 312, 1.2, 32, true, ["puzzle", "physics"]],
  ["Castle Sprint", "Lanternfly", 2007, 2, "works", "#60a917", 188, 3, 60, false, ["platformer"]],
  ["Neon Orbit", "Halfpixel", 2011, 3, "projector", "#6a00ff", 95, 5, 12, false, ["shooter", "arcade"]],
  ["Tiny Tower Defense", "Moss & Co", 2009, 2, "works", "#fa6800", 1430, 7, 90, true, ["strategy", "tower defense"]],
  ["Submarine Socks", "Wobble", 2006, 1, "works", "#00aba9", 41, 9, 9, false, ["adventure"]],
  ["Rooftop Courier", "Lanternfly", 2012, 3, "partial", "#e51400", 260, 14, 50, false, ["runner"]],
  ["Moonbase Bakery", "Spoonworks", 2009, 2, "works", "#647687", 67, 21, 70, false, ["management"]],
  ["Ice Cube Joust", "Halfpixel", 2008, 2, "works", "#0050ef", 22, 30, 30, true, ["versus", "arcade"]],
  ["Lantern Maze", "Moss & Co", 2005, 1, "works", "#825a2c", 0, null, 2, false, ["puzzle"]],
  ["Pocket Rally", "Kitebox", 2010, 3, "unknown", "#a20025", 0, null, 1, false, ["racing"]],
  ["Wizard Laundromat", "Wobble", 2011, 3, "works", "#76608a", 133, 45, 100, false, ["rpg"]],
];

export function sampleGames(): Game[] {
  return SEEDS.map(([title, developer, year, asVersion, compat, color, minutes, last, added, favorite, tags], i) => ({
    id: i + 1,
    title,
    developer,
    publisher: null,
    year,
    description: null,
    source: i % 3 === 0 ? "uploaded" : "archive",
    swfPath: `C:/sample/${title.toLowerCase().replace(/ /g, "_")}.swf`,
    originalName: `${title.toLowerCase().replace(/ /g, "_")}.swf`,
    sha256: String(i).padStart(64, "0"),
    fileSize: 400_000 + i * 123_457,
    swfVersion: asVersion === 3 ? 10 : asVersion === 2 ? 8 : 6,
    compression: asVersion === 3 ? "zlib" : "none",
    width: 550,
    height: 400,
    frameRate: 24,
    frameCount: 1,
    asVersion,
    usesNetwork: false,
    background: "#000000",
    compat,
    color,
    favorite,
    playSeconds: minutes * 60,
    lastPlayed: last === null ? null : ago(last),
    addedAt: ago(added),
    runtime: compat === "projector" ? "projector" : "ruffle",
    allowNetwork: false,
    tags,
    fallbackImage: null,
    scaleMode: "fit",
    quality: "high",
    frameRateOverride: null,
    letterbox: null,
    series: null,
    releaseDate: null,
    scrapedAt: null,
    titleLocked: false,
    art: {},
    links: [],
    contentId: null,
    launchUrl: null,
    gamepadMap: null,
  }));
}

export function sampleCollections(): Collection[] {
  return [
    { id: 1, name: "couch co-op", gameIds: [9, 2] },
    { id: 2, name: "school computer lab", gameIds: [3, 5, 10, 8] },
    { id: 3, name: "unfinished", gameIds: [4, 7] },
  ];
}
