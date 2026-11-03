// Where the Sudsy executable and the C# worker are. Free of `vscode` imports, so that the tests
// run under plain node.
import * as path from "node:path";

export interface ServerSettings {
  /** `sudsy.serverPath`; empty when unset. */
  serverPath: string;
  /** `sudsy.csharpWorkerPath`; empty when unset. */
  csharpWorkerPath: string;
}

export interface ServerCommand {
  command: string;
  args: string[];
  /** Variables to add to the environment of the server. */
  env: Record<string, string>;
}

/** The environment variable `sudsy` reads the worker location from. */
export const WORKER_ENV = "SUDSY_CSHARP_WORKER";

/** The executable the extension package bundles (see scripts/stage-server.mjs). */
export function bundledServer(extensionPath: string, platform: NodeJS.Platform): string {
  return path.join(extensionPath, "server", platform === "win32" ? "sudsy.exe" : "sudsy");
}

/** `${workspaceFolder}` in a setting is the first folder of the workspace. */
export function expand(value: string, workspaceFolder: string | undefined): string {
  return workspaceFolder === undefined ? value : value.replaceAll("${workspaceFolder}", workspaceFolder);
}

/**
 * How to start the server: the configured executable, else the bundled one, else `sudsy` from
 * PATH. The worker needs no setting when the executable is the bundled one: `sudsy` finds
 * `csharp-worker/Sudsy.Worker.dll` beside itself.
 */
export function resolveServer(
  settings: ServerSettings,
  extensionPath: string,
  platform: NodeJS.Platform,
  exists: (file: string) => boolean,
  workspaceFolder?: string,
): ServerCommand {
  const configured = expand(settings.serverPath.trim(), workspaceFolder);
  const bundled = bundledServer(extensionPath, platform);
  const command = configured !== "" ? configured : exists(bundled) ? bundled : "sudsy";
  const worker = expand(settings.csharpWorkerPath.trim(), workspaceFolder);
  return {
    command,
    args: ["lsp"],
    env: worker !== "" ? { [WORKER_ENV]: worker } : {},
  };
}
