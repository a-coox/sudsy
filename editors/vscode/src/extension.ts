import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import * as vscode from "vscode";
import { LanguageClient, type LanguageClientOptions } from "vscode-languageclient/node";
import { type RuleDoc, ruleMarkdown } from "./ruleDocs";
import { documentSelector } from "./selector.generated";
import { resolveServer } from "./server";

let client: LanguageClient | undefined;

const SETTINGS_THAT_RESTART = ["sudsy.serverPath", "sudsy.csharpWorkerPath"];

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  context.subscriptions.push(
    vscode.commands.registerCommand("sudsy.restartServer", () => restart(context)),
    vscode.commands.registerCommand("sudsy.showOutput", () => client?.outputChannel.show()),
    ...ruleDocs(context.extensionPath),
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

const DOCS_SCHEME = "sudsy-rule";

/** "Show documentation for <rule>" on Sudsy diagnostics, rendered from the bundled rules.json (offline, matches the installed version). */
function ruleDocs(extensionPath: string): vscode.Disposable[] {
  const rules = new Map<string, RuleDoc>();
  try {
    const file = JSON.parse(readFileSync(join(extensionPath, "schemas", "rules.json"), "utf8")) as { rules: RuleDoc[] };
    for (const r of file.rules) rules.set(r.id, r);
  } catch {
    // Without the file there are no rule pages and no quick fix.
  }
  return [
    vscode.workspace.registerTextDocumentContentProvider(DOCS_SCHEME, {
      provideTextDocumentContent: (uri) => {
        const rule = rules.get(uri.path.replace(/^\/|\.md$/g, ""));
        return rule ? ruleMarkdown(rule) : "";
      },
    }),
    vscode.commands.registerCommand("sudsy.showRuleDocs", (id: string) =>
      vscode.commands.executeCommand("markdown.showPreview", vscode.Uri.parse(`${DOCS_SCHEME}:/${id}.md`)),
    ),
    vscode.languages.registerCodeActionsProvider(
      documentSelector,
      {
        provideCodeActions: (_document, _range, context) =>
          context.diagnostics.flatMap((d) => {
            const id = typeof d.code === "object" ? String(d.code.value) : String(d.code);
            if (d.source !== "sudsy" || !rules.has(id)) return [];
            const action = new vscode.CodeAction(`Show documentation for ${id}`, vscode.CodeActionKind.QuickFix);
            action.command = { command: "sudsy.showRuleDocs", title: action.title, arguments: [id] };
            action.diagnostics = [d];
            return [action];
          }),
      },
      { providedCodeActionKinds: [vscode.CodeActionKind.QuickFix] },
    ),
  ];
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
