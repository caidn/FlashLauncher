// One-time setup for in-app updates. Run it once on your own computer:
//
//   npm run setup-updater
//
// It makes the key that signs every release (so the app only ever installs
// updates that really came from you), keeps the private half in
// ~/.tauri/ (never in the repo), writes the public half into
// src-tauri/tauri.conf.json, and, if the GitHub CLI is installed and logged
// in, saves the private key and its password as repository secrets for the
// release workflow. Otherwise it tells you what to paste where.
//
// Lose the key or password and existing installs can't update anymore, so
// back up ~/.tauri/flash-launcher.key and .password somewhere safe.

import { execFileSync, spawnSync } from "node:child_process";
import { randomBytes } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const dir = join(homedir(), ".tauri");
const keyPath = join(dir, "flash-launcher.key");
const passPath = join(dir, "flash-launcher.password");
const confPath = join(root, "src-tauri", "tauri.conf.json");
const npx = process.platform === "win32" ? "npx.cmd" : "npx";

mkdirSync(dir, { recursive: true });

if (existsSync(keyPath)) {
  console.log(`Using the existing key at ${keyPath}`);
} else {
  const password = randomBytes(24).toString("base64url");
  writeFileSync(passPath, password, { mode: 0o600 });
  const r = spawnSync(npx, ["tauri", "signer", "generate", "--ci", "-w", keyPath, "-p", password], {
    cwd: root,
    stdio: ["ignore", "ignore", "inherit"],
    shell: process.platform === "win32",
  });
  if (r.status !== 0 || !existsSync(keyPath)) {
    console.error("Couldn't generate the key. Is `npm install` done?");
    process.exit(1);
  }
  console.log(`Made a new signing key: ${keyPath}`);
}

const pubkey = readFileSync(`${keyPath}.pub`, "utf8").trim();
const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.plugins ??= {};
conf.plugins.updater ??= {};
if (conf.plugins.updater.pubkey !== pubkey) {
  conf.plugins.updater.pubkey = pubkey;
  writeFileSync(confPath, JSON.stringify(conf, null, 2) + "\n");
  console.log("Wrote the public key into src-tauri/tauri.conf.json (commit that file).");
}

const privateKey = readFileSync(keyPath, "utf8").trim();
const password = existsSync(passPath) ? readFileSync(passPath, "utf8").trim() : "";

function hasGh() {
  try {
    execFileSync("gh", ["auth", "status"], { stdio: "ignore", shell: process.platform === "win32" });
    return true;
  } catch {
    return false;
  }
}

if (hasGh()) {
  const set = (name, value) =>
    execFileSync("gh", ["secret", "set", name], {
      cwd: root,
      input: value,
      stdio: ["pipe", "inherit", "inherit"],
      shell: process.platform === "win32",
    });
  set("TAURI_SIGNING_PRIVATE_KEY", privateKey);
  set("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", password);
  console.log("Saved TAURI_SIGNING_PRIVATE_KEY and TAURI_SIGNING_PRIVATE_KEY_PASSWORD as GitHub secrets.");
} else {
  console.log(`
The GitHub CLI isn't set up, so add two secrets by hand:
GitHub repo -> Settings -> Secrets and variables -> Actions -> New repository secret

  TAURI_SIGNING_PRIVATE_KEY           the contents of ${keyPath}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD  the contents of ${passPath}
`);
}

console.log(`Back up ${keyPath} and ${passPath}. Without them, installed copies can't be updated.`);
