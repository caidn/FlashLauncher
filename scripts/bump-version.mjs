// Sets the app version everywhere it lives: npm run bump 0.8.0
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const v = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(v ?? "")) {
  console.error("Usage: npm run bump 0.8.0");
  process.exit(1);
}
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const edit = (rel, fn) => {
  const p = join(root, rel);
  writeFileSync(p, fn(readFileSync(p, "utf8")));
};
edit("package.json", (s) => s.replace(/("version":\s*")[^"]+"/, `$1${v}"`));
edit("src-tauri/tauri.conf.json", (s) => s.replace(/("version":\s*")[^"]+"/, `$1${v}"`));
edit("src-tauri/Cargo.toml", (s) => s.replace(/^version = "[^"]+"/m, `version = "${v}"`));
console.log(`Version is now ${v}. Commit, then: git tag v${v} && git push origin v${v}`);
