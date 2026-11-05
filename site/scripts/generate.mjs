// Writes the data-driven pages (rules, upstream-id stubs, presets, format options) as Markdown into
// src/content/docs/generated/ (git-ignored). Sources: docs/rules/rules.json, schemas/sudsy.schema.json,
// crates/lang-*/presets/*.yaml and the notes column of docs/formatting/presets.md.
import { mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { base, site, rules } from '../config.mjs';

const root = fileURLToPath(new URL('../../', import.meta.url));
const out = fileURLToPath(new URL('../src/content/docs/generated/', import.meta.url));
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

function example(e) {
  if (!e) return '';
  const lines = e.input.replace(/\n$/, '').split('\n');
  const marked = lines.flatMap((l, i) => {
    const sp = e.spans.filter((s) => s.line === i + 1);
    if (!sp.length) return [l];
    const marks = Array(Math.max(...sp.map((s) => s.column - 1 + s.length))).fill(' ');
    for (const s of sp) marks.fill('^', s.column - 1, s.column - 1 + s.length);
    return [l, marks.join('')];
  });
  return [
    '## Example',
    e.config ? 'Configuration:\n\n' + code('yaml', e.config) : '',
    'Input, with the reported spans marked:\n\n' + code('text', marked.join('\n')),
    e.fixed ? 'After the fix:\n\n' + code(e.language, e.fixed) : '',
  ].filter(Boolean).join('\n\n');
}

const ruleIds = new Set(rules().map((r) => r.id.toLowerCase()));
const stubs = new Map(); // slug -> { id, tools, rules }
for (const r of rules()) {
  const chips = [
    ...r.languages.map((v) => chip('language', v)),
    chip('category', r.category),
    chip('level', r.default_level),
    chip('fix', r.fix ?? 'none'),
    ...r.presets.map((v) => chip('preset', v)),
    ...[...new Set(r.upstream.map((u) => u.tool))].map((v) => chip('upstream', v)),
  ];
  const body = [
    `<div class="meta">${chips.join('')}</div>`,
    md(r.description),
    r.options.length &&
      '## Options\n\n' +
        table(['Option', 'Type', 'Default', 'Values', 'Description'], r.options.map((o) => [`\`${o.name}\``, o.type, fmt(o.default), o.choices ? o.choices.map((c) => `\`${c}\``).join(', ') : '', cell(o.description)])),
    r.upstream.length &&
      '## Upstream equivalents\n\n' +
        table(['Tool', 'Id', 'Coverage'], r.upstream.map((u) => [u.tool, `\`${u.id}\``, u.partial ? 'partial' : 'full'])),
    r.presets.length && '## Presets\n\nEnabled by: ' + r.presets.map((p) => `[\`${p}\`](${base}/reference/presets/#${p.replace('/', '')})`).join(', '),
    `<div class="example">\n\n${example(r.example)}\n\n</div>`,
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
for (const [key, s] of stubs) {
  const ids = [...s.rules];
  const tools = [...s.tools].join(', ');
  const front = { title: s.id, slug: `rules/${s.slug}`, description: `${tools} ${s.id} in Sudsy: ${ids.join(', ')}.` };
  if (ids.length === 1) {
    front.head = [
      { tag: 'meta', attrs: { 'http-equiv': 'refresh', content: `0; url=${url(ids[0])}` } },
      { tag: 'link', attrs: { rel: 'canonical', href: site + url(ids[0]) } },
    ];
  }
  put(`up-${key}.md`, front, `${tools} \`${s.id}\` is covered by ${ids.map((i) => `[${i}](${url(i)})`).join(', ')}.`);
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
