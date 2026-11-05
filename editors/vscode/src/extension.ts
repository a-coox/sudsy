import { existsSync } from "node:fs";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions } from "vscode-languageclient/node";
import { documentSelector } from "./selector.generated";
import { resolveServer } from "./server";

let client: LanguageClient | undefined;

const SETTINGS_THAT_RESTART = ["sudsy.serverPath", "sudsy.csharpWorkerPath"];

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("sudsy.restartServer", () => restart(context)),
    vscode.commands.registerCommand("sudsy.showOutput", () => client?.outputChannel.show()),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (SETTINGS_THAT_RESTART.some((s) => e.affectsConfiguration(s))) {
        void restart(context);
      }
    }),
  );
  await start(context);
}

export async function deactivate(): Promise<void> {
  await stop();
}

async function start(context: vscode.ExtensionContext): Promise<void> {
  const settings = vscode.workspace.getConfiguration("sudsy");
  const server = resolveServer(
    {
      serverPath: settings.get<string>("serverPath", ""),
      csharpWorkerPath: settings.get<string>("csharpWorkerPath", ""),
    },
    context.extensionPath,
    process.platform,
    existsSync,
    vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
  );
  const options: LanguageClientOptions = {
    documentSelector,
    // Pull diagnostics for every open tab, not just the visible editor.
    diagnosticPullOptions: { onChange: true, onSave: true, onTabs: true },
    outputChannelName: "Sudsy",
  };
  client = new LanguageClient(
    "sudsy",
    "Sudsy",
    {
      command: server.command,
      args: server.args,
      options: { env: { ...process.env, ...server.env } },
    },
    options,
  );
  try {
    await client.start();
  } catch (e) {
    client = undefined;
    void vscode.window.showErrorMessage(
      `Sudsy could not start \`${server.command}\`: ${e instanceof Error ? e.message : String(e)}. ` +
        "Set `sudsy.serverPath` or put `sudsy` on PATH.",
    );
  }
}

async function stop(): Promise<void> {
  const running = client;
  client = undefined;
  await running?.stop();
}

async function restart(context: vscode.ExtensionContext): Promise<void> {
  await stop();
  await start(context);
}
