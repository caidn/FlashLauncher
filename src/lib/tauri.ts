// Thin wrapper so the UI also runs in a plain browser (npm run dev) where
// the Tauri APIs don't exist.

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function win() {
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  return getCurrentWindow();
}

export const windowControls = {
  async minimize() {
    if (isTauri) await (await win()).minimize();
  },
  async toggleMaximize() {
    if (isTauri) await (await win()).toggleMaximize();
  },
  async close() {
    if (isTauri) await (await win()).close();
  },
  async isMaximized(): Promise<boolean> {
    return isTauri ? (await win()).isMaximized() : false;
  },
  async onResized(cb: () => void): Promise<() => void> {
    if (!isTauri) return () => {};
    return (await win()).onResized(cb);
  },
};
