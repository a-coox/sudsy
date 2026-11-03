// Usage: node scripts/stage-server.mjs <path of the sudsy executable> <directory of the published
// C# worker>. Lays out server/ as the extension expects it: the executable with the worker in
// csharp-worker/ beside it, which is where `sudsy` looks for it.
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const [exe, worker] = process.argv.slice(2);
if (!exe || !worker) {
  console.error("usage: stage-server.mjs <sudsy executable> <published worker directory>");
  process.exit(2);
}
const server = join(dirname(fileURLToPath(import.meta.url)), "../server");
rmSync(server, { recursive: true, force: true });
mkdirSync(server, { recursive: true });
cpSync(exe, join(server, basename(exe)));
cpSync(worker, join(server, "csharp-worker"), { recursive: true });
