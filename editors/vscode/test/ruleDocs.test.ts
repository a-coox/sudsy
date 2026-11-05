import assert from "node:assert/strict";
import { test } from "node:test";
import { ruleMarkdown, type RuleDoc } from "../src/ruleDocs";

const rule: RuleDoc = {
  id: "no-empty-catch",
  languages: ["csharp"],
  category: "suspicious",
  default_level: "warn",
  fix: null,
  description: "A catch block with no statements.",
  options: [{ name: "max", type: "int", default: 3, description: "Limit." }],
  presets: ["csharp/stylecop"],
  upstream: [{ tool: "Roslynator", id: "RCS1075", partial: false }],
  example: { language: "csharp", input: "try { } catch { }\n", spans: [{ line: 1, column: 9, length: 5 }], fixed: null },
};

test("the page names the rule, its presets, equivalents, options and example", () => {
  const md = ruleMarkdown(rule);
  for (const part of ["# no-empty-catch", "`csharp/stylecop`", "Roslynator RCS1075", "`max` (int, default `3`)", "```csharp", "line 1, column 9"]) {
    assert.ok(md.includes(part), part);
  }
  assert.ok(!md.includes("After the fix"));
});
