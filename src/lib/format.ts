export function formatPlayTime(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  if (minutes <= 0) return seconds > 0 ? "under a minute" : "never played";
  if (minutes < 60) return `${minutes}m`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m ? `${h}h ${m}m` : `${h}h`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function formatFps(fps: number): string {
  return `${Number.isInteger(fps) ? fps : fps.toFixed(1)} fps`;
}

export function timeAgo(iso: string | null, now = Date.now()): string {
  if (!iso) return "never";
  const s = Math.max(0, (now - new Date(iso).getTime()) / 1000);
  if (s < 60) return "just now";
  const m = s / 60;
  if (m < 60) return `${Math.floor(m)} min ago`;
  const h = m / 60;
  if (h < 24) return `${Math.floor(h)} hr ago`;
  const d = h / 24;
  if (d < 2) return "yesterday";
  if (d < 30) return `${Math.floor(d)} days ago`;
  const mo = d / 30;
  if (mo < 12) return `${Math.floor(mo)} mo ago`;
  return `${Math.floor(mo / 12)} yr ago`;
}
