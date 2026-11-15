// Flash SharedObjects (the saves) live in localStorage while a game runs,
// because that's where Ruffle keeps them. The library DB is the real
// store: before a game starts we load only its saves into localStorage,
// and we copy them back out while it runs and when it stops. That keeps
// games from seeing (or clobbering) each other's saves, and lets saves
// survive wiping the WebView's storage.

const OWN_PREFIX = "flashlauncher.";

function gameKeys(): string[] {
  const keys: string[] = [];
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const k = localStorage.key(i);
      if (k && !k.startsWith(OWN_PREFIX)) keys.push(k);
    }
  } catch {
    /* storage unavailable */
  }
  return keys;
}

/** Removes whatever game data is lying around in localStorage. */
export function clearGameStorage() {
  try {
    for (const k of gameKeys()) localStorage.removeItem(k);
  } catch {
    /* ignore */
  }
}

/** Puts one game's saves into localStorage, replacing anything else. */
export function restoreSaves(json: string | null) {
  clearGameStorage();
  if (!json) return;
  try {
    const data = JSON.parse(json) as Record<string, string>;
    for (const [k, v] of Object.entries(data)) localStorage.setItem(k, v);
  } catch {
    /* corrupt blob: start fresh rather than crash the game */
  }
}

/** The current game's saves as a JSON blob for the DB. */
export function snapshotSaves(): string {
  const out: Record<string, string> = {};
  try {
    for (const k of gameKeys()) {
      const v = localStorage.getItem(k);
      if (v !== null) out[k] = v;
    }
  } catch {
    /* ignore */
  }
  return JSON.stringify(out);
}
