// Loads the self-hosted Ruffle build (public/ruffle, copied from npm by
// scripts/copy-ruffle.mjs) and types the small slice of its API we use.

export interface RuffleConfig {
  autoplay?: "on" | "off" | "auto";
  unmuteOverlay?: "visible" | "hidden";
  splashScreen?: boolean;
  preloader?: boolean;
  letterbox?: "off" | "fullscreen" | "on";
  scale?: "showAll" | "exactFit" | "noBorder" | "noScale";
  forceScale?: boolean;
  quality?: "low" | "medium" | "high" | "best";
  frameRate?: number | null;
  allowNetworking?: "all" | "internal" | "none";
  openUrlMode?: "allow" | "confirm" | "deny";
  contextMenu?: "on" | "rightClickOnly" | "off";
  warnOnUnsupportedContent?: boolean;
  showSwfDownload?: boolean;
  allowFullscreen?: boolean;
  logLevel?: "error" | "warn" | "info" | "debug" | "trace";
  backgroundColor?: string | null;
  polyfills?: boolean;
  publicPath?: string;
  /** [pattern, replacement] pairs applied to every URL the movie loads. */
  urlRewriteRules?: [RegExp | string, string][];
  /** Ruffle button name -> Flash key code. */
  gamepadButtonMapping?: Record<string, number>;
  parameters?: Record<string, string> | string;
  base?: string;
}

export interface RuffleMetadata {
  width: number;
  height: number;
  frameRate: number;
  isActionScript3: boolean;
  swfVersion: number;
}

/** Version 1 of Ruffle's player API, from `element.ruffle()`. */
export interface RuffleApi {
  load(opts: RuffleConfig & ({ data: ArrayBuffer; swfFileName?: string } | { url: string })): Promise<void>;
  reload(): Promise<void>;
  suspend(): void;
  resume(): void;
  readonly suspended: boolean;
  readonly isPlaying: boolean;
  readonly metadata: RuffleMetadata | null;
  volume: number;
}

export interface RufflePlayerElement extends HTMLElement {
  ruffle(version?: 1): RuffleApi;
}

interface RuffleGlobal {
  config?: RuffleConfig;
  newest(): { createPlayer(): RufflePlayerElement };
}

declare global {
  interface Window {
    RufflePlayer?: RuffleGlobal;
  }
}

let loading: Promise<RuffleGlobal> | null = null;

/**
 * WebGL clears its drawing buffer after every frame unless asked not to,
 * which makes screenshots come out blank on some engines (WebKit always,
 * Chromium sometimes). Ruffle doesn't expose the option, so default it on
 * for every WebGL context created from here on. The cost is negligible
 * for 2D-era Flash content.
 */
function keepDrawingBuffers() {
  const proto = HTMLCanvasElement.prototype as unknown as {
    getContext: (type: string, attrs?: Record<string, unknown>) => RenderingContext | null;
    __keepBuffer?: boolean;
  };
  if (proto.__keepBuffer) return;
  const original = proto.getContext;
  proto.getContext = function (this: HTMLCanvasElement, type: string, attrs?: Record<string, unknown>) {
    if (type === "webgl" || type === "webgl2" || type === "experimental-webgl") {
      attrs = { ...attrs, preserveDrawingBuffer: true };
    }
    return original.call(this, type, attrs);
  };
  proto.__keepBuffer = true;
}

export function loadRuffle(): Promise<RuffleGlobal> {
  if (loading) return loading;
  keepDrawingBuffers();
  loading = new Promise((resolve, reject) => {
    // Global config must exist before the script runs. No polyfilling:
    // we create players ourselves, there are no <embed>s to take over.
    window.RufflePlayer = Object.assign(window.RufflePlayer ?? {}, {
      config: { polyfills: false, publicPath: "/ruffle/" },
    }) as RuffleGlobal;
    const s = document.createElement("script");
    s.src = "/ruffle/ruffle.js";
    s.async = true;
    s.onload = () => {
      const r = window.RufflePlayer;
      if (r && typeof r.newest === "function") resolve(r);
      else reject(new Error("Ruffle loaded but didn't start."));
    };
    s.onerror = () => {
      loading = null;
      reject(new Error("Couldn't load Ruffle. Run npm install, then restart the app."));
    };
    document.head.appendChild(s);
  });
  return loading;
}

/**
 * Grabs the current frame as a PNG. Ruffle draws with WebGL, whose buffer
 * is only readable in the same frame it was drawn, so this reads right
 * after the next paint.
 */
export function captureFrame(player: HTMLElement): Promise<Blob> {
  return new Promise((resolve, reject) => {
    requestAnimationFrame(() => {
      const canvas = player.shadowRoot?.querySelector("canvas");
      if (!canvas) return reject(new Error("no game canvas to capture"));
      if (isBlank(canvas)) return reject(new Error("the game's picture couldn't be read"));
      canvas.toBlob((b) => (b ? resolve(b) : reject(new Error("capture failed"))), "image/png");
    });
  });
}

/** True when the canvas reads back fully transparent (a failed capture). */
function isBlank(src: HTMLCanvasElement): boolean {
  const c = document.createElement("canvas");
  c.width = 32;
  c.height = 32;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return false;
  ctx.drawImage(src, 0, 0, 32, 32);
  const px = ctx.getImageData(0, 0, 32, 32).data;
  for (let i = 3; i < px.length; i += 4) if (px[i] !== 0) return false;
  return true;
}
