// Copies the self-hosted Ruffle build from node_modules into public/ruffle,
// where Vite serves it in dev and bundles it into the app on build.
// Runs automatically before `npm run dev` and `npm run build`.
//
// To update Ruffle: npm install --save-exact @ruffle-rs/ruffle@<version>
// (nightlies are tagged "nightly" on npm and are usually the better pick
// for AS3 games).

import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src = join(root, "node_modules", "@ruffle-rs", "ruffle");
const dest = join(root, "public", "ruffle");

if (!existsSync(join(src, "package.json"))) {
  console.error("Ruffle isn't installed. Run `npm install` first.");
  process.exit(1);
}

const version = JSON.parse(readFileSync(join(src, "package.json"), "utf8")).version;
const stamp = join(dest, ".version");
if (existsSync(stamp) && readFileSync(stamp, "utf8") === version) {
  process.exit(0); // already up to date
}

rmSync(dest, { recursive: true, force: true });
mkdirSync(dest, { recursive: true });
for (const f of readdirSync(src)) {
  if (f.endsWith(".js") || f.endsWith(".wasm") || f.startsWith("LICENSE")) {
    copyFileSync(join(src, f), join(dest, f));
  }
}
writeFileSync(stamp, version);
console.log(`Ruffle ${version} copied to public/ruffle`);
