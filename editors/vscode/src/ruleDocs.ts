// The documentation page of a rule, as Markdown, from docs/rules/rules.json (bundled with the extension).

/** Where the rule pages live once the documentation site is published; switch `codeDescription` links on with it. */
export const DOCS_BASE_URL = "https://a-coox.github.io/sudsy/rules/";

export interface RuleDoc {
  id: string;
  languages: string[];
  category: string;
  default_level: string;
  fix: string | null;
  description: string;
  options: { name: string; type: string; default: unknown; description: string; choices?: string[] }[];
  presets: string[];
  upstream: { tool: string; id: string; partial: boolean }[];
  example?: {
    language: string;
    input: string;
    spans: { line: number; column: number; length: number }[];
    fixed: string | null;
  };
}

export function ruleMarkdown(r: RuleDoc): string {
  const out = [
    `# ${r.id}`,
    "",
    r.description,
    "",
    `- Languages: ${r.languages.join(", ")}`,
    `- Category: ${r.category}; default level: ${r.default_level}; fix: ${r.fix ?? "none"}`,
  ];
  if (r.presets.length) out.push(`- Enabled by: ${r.presets.map((p) => `\`${p}\``).join(", ")}`);
  if (r.upstream.length) {
    out.push(`- Equivalent: ${r.upstream.map((u) => `${u.tool} ${u.id}${u.partial ? " (partial)" : ""}`).join(", ")}`);
  }
  if (r.options.length) {
    out.push("", "## Options", "");
    for (const o of r.options) {
      const choices = o.choices ? `; one of ${o.choices.map((c) => `\`${c}\``).join(", ")}` : "";
      out.push(`- \`${o.name}\` (${o.type}, default \`${JSON.stringify(o.default)}\`${choices}): ${o.description}`);
    }
  }
  const e = r.example;
  if (e) {
    const fence = "```" + e.language;
    out.push("", "## Example", "", fence, ...e.input.trimEnd().split("\n"), "```");
    out.push("", `Reported at ${e.spans.map((s) => `line ${s.line}, column ${s.column}`).join("; ")}.`);
    if (e.fixed !== null) out.push("", "After the fix:", "", fence, ...e.fixed.trimEnd().split("\n"), "```");
  }
  return out.join("\n") + "\n";
}
