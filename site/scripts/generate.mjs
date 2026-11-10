// Writes the data-driven pages (rules, upstream-id stubs, presets, format options) as Markdown into
// src/content/docs/generated/ (git-ignored). Sources: docs/rules/rules.json, schemas/sudsy.schema.json,
// crates/lang-*/presets/*.yaml and the notes column of docs/formatting/presets.md.
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { base, site, rules } from '../config.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const out = fileURLToPath(new URL('../src/content/docs/generated/', import.meta.url));
const pub = fileURLToPath(new URL('../public/', import.meta.url));
mkdirSync(pub, { recursive: true });
copyFileSync(root + 'assets/logo/soap_color.svg', pub + 'favicon.svg'); // git-ignored copy
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const put = (file, front, body) =>
  writeFileSync(out + file, `---\n${Object.entries(front).map(([k, v]) => `${k}: ${JSON.stringify(v)}`).join('\n')}\n---\n\n${body}\n`);
// Raw HTML in a description must not open a tag: escape `<` outside code spans; one line for table cells.
const md = (t) => t.split(/(`[^`]*`)/).map((s, i) => (i % 2 ? s : s.replace(/</g, '&lt;'))).join('').replace(/\s+/g, ' ').trim();
const cell = (t) => md(String(t)).replace(/\|/g, '\\|');
const table = (head, rows) => `| ${head.join(' | ')} |\n|${head.map(() => '---').join('|')}|\n${rows.map((r) => `| ${r.join(' | ')} |`).join('\n')}`;
const code = (lang, text) => '```' + lang + '\n' + text.replace(/\n$/, '') + '\n```';
const url = (slug) => `${base}/rules/${slug}/`;
const fmt = (v) => (v === undefined ? '' : `\`${JSON.stringify(v)}\``);

// Pagefind filter chip: the visible text names the filter, the attribute carries the value.
const chip = (name, value) => `<span data-pagefind-filter="${name}[data-v]" data-v="${value}">${name}: ${value}</span>`;

// Rule config as sudsy.yaml takes it: `lint.rules` at the top level for rules of all languages,
// under the language section otherwise (docs/config/sudsy-yaml.md).
function configuration(r) {
  const langs = r.languages.includes('all') ? [null] : r.languages;
  const indent = (t, n) => t.replace(/^/gm, ' '.repeat(n));
  const yaml = (entry) =>
    langs.map((l) => (l ? `${l}:\n  lint:\n    rules:\n${indent(entry, 6)}` : `lint:\n  rules:\n${indent(entry, 4)}`)).join('\n');
  const opts = r.options.filter((o) => o.default !== undefined);
  const levels = ['off', 'warn', 'error'].map((l) => (l === r.default_level ? `${l} (default)` : l)).join(', ');
  const entry = `${r.id}:\n  level: ${r.default_level === 'off' ? 'warn' : r.default_level}  # ${levels}` +
    (opts.length ? `\n  options:\n${opts.map((o) => `    ${o.name}: ${JSON.stringify(o.default)}`).join('\n')}` : '');
  return [
    '## Configuration',
    code('yaml', yaml(entry)),
    r.options.length &&
      table(['Option', 'Type', 'Default', 'Values', 'Description'], r.options.map((o) => [`\`${o.name}\``, o.type, fmt(o.default), o.choices ? o.choices.map((c) => `\`${c}\``).join(', ') : '', cell(o.description)])),
    r.presets.length && 'Enabled by the presets: ' + r.presets.map((p) => `[\`${p}\`](${base}/reference/presets/#${p.replace('/', '')})`).join(', ') + '.',
    `Suppress one line with \`sudsy-disable-next-line ${r.id}\` in a comment of the file's language.`,
  ].filter(Boolean).join('\n\n');
}

function example(e, id) {
  if (!e) return '';
  const lines = e.input.replace(/\n$/, '').split('\n');
  const squiggles = e.spans.map((s) => `${s.line}:${s.column}:${s.length}`).join(',');
  return [
    '## Example',
    e.config ? 'Configuration:\n\n' + code('yaml', e.config) : '',
    code(e.language, lines.join('\n')).replace(/^```(\S*)/, `\`\`\`$1 squiggles="${squiggles}" tip="${id}"`),
    e.fixed ? 'After the fix:\n\n' + code(e.language, e.fixed) : '',
  ].filter(Boolean).join('\n\n');
}

const realLanguages = [...new Set(rules().flatMap((r) => r.languages))].filter((l) => l !== 'all');
const ruleIds =new Set(rules().map((r) => r.id.toLowerCase()));
const stubs = new Map(); // slug -> { id, tools, rules }
for (const r of rules()) {
  const chips = [
    // A rule for all languages shows "all" but is filterable under every real language.
    ...r.languages.map((v) => (v === 'all' ? `<span>language: all</span>` : chip('language', v))),
    ...(r.languages.includes('all') ? realLanguages.map((v) => `<span hidden data-pagefind-filter="language[data-v]" data-v="${v}"></span>`) : []),
    chip('category', r.category),
    chip('fix', r.fix ?? 'none'),
    ...r.presets.map((v) => chip('preset', v)),
    ...[...new Set(r.upstream.map((u) => u.tool))].map((v) => chip('upstream', v)),
  ];
  const body = [
    `<div class="meta" data-pagefind-sort="id[data-id]" data-id="${r.id}">${chips.join('')}</div>`,
    md(r.description),
    r.upstream.length &&
      '## Upstream equivalents\n\n' +
        table(['Tool', 'Id', 'Coverage'], r.upstream.map((u) => [u.tool, `\`${u.id}\``, u.partial ? 'partial' : 'full'])),
    `<div class="example">\n\n${example(r.example, r.id)}\n\n</div>`,
    configuration(r),
  ].filter(Boolean).join('\n\n');
  put(`rule-${r.id}.md`, { title: r.id, slug: `rules/${r.id}`, description: md(r.description).slice(0, 160) }, body);

  for (const u of r.upstream) {
    const slug = u.id.replace(/[^A-Za-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '');
    if (!slug || ruleIds.has(slug.toLowerCase())) continue;
    const s = stubs.get(slug.toLowerCase()) ?? { slug, id: u.id, tools: new Set(), rules: new Set() };
    s.tools.add(u.tool);
    s.rules.add(r.id);
    stubs.set(slug.toLowerCase(), s);
  }
}

// One Sudsy rule: redirect to it. Several: list them (a redirect would hide the others).
// Plain static files in public/ (a Starlight page would carry 19 KB of layout each).
rmSync(pub + 'rules', { recursive: true, force: true });
for (const s of stubs.values()) {
  const ids = [...s.rules];
  const tools = [...s.tools].join(', ');
  const head = ids.length === 1
    ? `<meta http-equiv="refresh" content="0; url=${url(ids[0])}"><link rel="canonical" href="${site + url(ids[0])}">`
    : '';
  mkdirSync(`${pub}rules/${s.slug}`, { recursive: true });
  writeFileSync(`${pub}rules/${s.slug}/index.html`,
    `<!doctype html><meta charset="utf-8"><title>${s.id} | Sudsy</title>${head}<body>${tools} <code>${s.id}</code> is covered by ${ids.map((i) => `<a href="${url(i)}">${i}</a>`).join(', ')}.\n`);
}

// Presets: the notes of docs/formatting/presets.md next to each preset file.
const notes = new Map();
for (const m of readFileSync(root + 'docs/formatting/presets.md', 'utf8').matchAll(/^\| `([a-z]+\/[\w-]+)` \| (.*) \|$/gm)) {
  notes.set(m[1], m[2].replace(/\[([^\]]*)\]\([^)]*\)/g, '$1'));
}
const presets = [];
for (const dir of readdirSync(root + 'crates').filter((d) => existsSync(`${root}crates/${d}/presets`))) {
  for (const f of readdirSync(`${root}crates/${dir}/presets`)) {
    presets.push([`${dir.replace('lang-', '')}/${f.replace('.yaml', '')}`, readFileSync(`${root}crates/${dir}/presets/${f}`, 'utf8')]);
  }
}
put('presets.md', { title: 'Presets', slug: 'reference/presets', description: 'Built-in presets for `extends: [preset:<language>/<name>]`.' },
  'A preset is an ordinary `sudsy.yaml`; writing its options by hand gives the same result. Use one with `extends: [preset:<language>/<name>]`.\n\n' +
    presets.sort(([a], [b]) => a.localeCompare(b)).map(([name, yaml]) =>
      `## ${name}\n\n${md(notes.get(name) ?? '')}\n\n[Rules it enables](${base}/rules/?preset=${name})\n\n${code('yaml', yaml)}`).join('\n\n'));

// Format options: every language's `format` section of the schema.
const schema = JSON.parse(readFileSync(root + 'schemas/sudsy.schema.json', 'utf8'));
const sections = Object.entries(schema.properties)
  .filter(([, v]) => v.properties?.format)
  .map(([lang, v]) => [lang, v.properties.format.properties]);
put('format-options.md', { title: 'Format options', slug: 'reference/format-options', description: 'Every formatting option per language, with its type, default and allowed values.' },
  'Set under `<language>.format` in `sudsy.yaml`. The options `indent_style`, `indent_size`, `line_width`, `end_of_line`, `final_newline`, `bracket_spacing` and `array_bracket_spacing` can also be set once under a top-level `format`.\n\n' +
    sections.map(([lang, props]) =>
      `## ${lang}\n\n` + table(['Option', 'Type', 'Default', 'Values', 'Description'],
        Object.entries(props).sort(([a], [b]) => a.localeCompare(b)).map(([n, o]) =>
          [`\`${n}\``, o.type, fmt(o.default), o.enum ? o.enum.map((c) => `\`${c}\``).join(', ') : o.minimum !== undefined ? `${o.minimum}..${o.maximum ?? ''}` : '', cell(o.description ?? '')]))).join('\n\n'));

console.log(`generated ${rules().length} rules, ${stubs.size} upstream stubs, ${presets.length} presets`);
