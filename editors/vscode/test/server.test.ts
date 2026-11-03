import assert from "node:assert/strict";
import { test } from "node:test";
import * as path from "node:path";
import { WORKER_ENV, bundledServer, resolveServer } from "../src/server";

const none = { serverPath: "", csharpWorkerPath: "" };
const ext = path.join("ext", "sudsy");

test("the bundled executable is named for the platform", () => {
  assert.equal(bundledServer(ext, "win32"), path.join(ext, "server", "sudsy.exe"));
  assert.equal(bundledServer(ext, "linux"), path.join(ext, "server", "sudsy"));
});

test("a configured executable wins over the bundled one", () => {
  const s = resolveServer({ ...none, serverPath: " /opt/sudsy " }, ext, "linux", () => true);
  assert.deepEqual(s, { command: "/opt/sudsy", args: ["lsp"], env: {} });
});

test("the bundled executable is used when it exists, else PATH", () => {
  const bundled = bundledServer(ext, "linux");
  assert.equal(resolveServer(none, ext, "linux", (f) => f === bundled).command, bundled);
  assert.equal(resolveServer(none, ext, "linux", () => false).command, "sudsy");
});

test("the worker setting becomes the environment variable the executable reads", () => {
  const s = resolveServer({ ...none, csharpWorkerPath: "/w/Sudsy.Worker.dll" }, ext, "linux", () => false);
  assert.deepEqual(s.env, { [WORKER_ENV]: "/w/Sudsy.Worker.dll" });
});

test("${workspaceFolder} in the settings is the first workspace folder", () => {
  const s = resolveServer(
    { serverPath: "${workspaceFolder}/target/sudsy", csharpWorkerPath: "${workspaceFolder}/w.dll" },
    ext,
    "linux",
    () => false,
    "/repo",
  );
  assert.equal(s.command, "/repo/target/sudsy");
  assert.deepEqual(s.env, { [WORKER_ENV]: "/repo/w.dll" });
});
