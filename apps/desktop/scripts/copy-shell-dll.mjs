// Copies the compact-context-menu handler (crates/bolt-shell) to where Bolt expects it:
//  - src-tauri/shellext/  -> bundled into the installer (tauri.conf.json "resources")
//  - target/release, target/debug (next to the exe) -> so a local build / `cargo tauri dev` can register it
// Run after `cargo build --release -p bolt-shell` (the beforeBuildCommand does both).
import { copyFileSync, existsSync, mkdirSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const app = resolve(here, '..');
const built = resolve(app, '../../target/release/bolt_shell.dll');
if (!existsSync(built)) {
  console.error(`missing ${built}; run: cargo build --release -p bolt-shell`);
  process.exit(1);
}
const bundled = join(app, 'src-tauri', 'shellext', 'bolt_shell.dll');
mkdirSync(dirname(bundled), { recursive: true });
copyFileSync(built, bundled);
for (const profile of ['debug', 'release']) {
  const dir = resolve(app, '../../target', profile);
  if (existsSync(dir)) {
    copyFileSync(built, join(dir, 'bolt_shell.dll'));
    copyFileSync(join(app, 'src-tauri', 'icons', 'icon.ico'), join(dir, 'bolt.ico'));
  }
}
console.log('bolt_shell.dll copied');
