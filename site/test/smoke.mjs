// Browser smoke test of the built site (site/dist): `scripts/site.sh test`. Needs Edge or Chrome installed.
import { after, before, test } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { chromium } from 'playwright-core';

const PORT = 4399;
const B = `http://localhost:${PORT}/sudsy`;
const server = spawn('npx', ['astro', 'preview', '--port', String(PORT)], { shell: true, stdio: 'ignore' });
let browser, page;
const problems = [];

before(async () => {
  for (let i = 0; i < 100; i++) {
    if (await fetch(`${B}/`).then((r) => r.ok, () => false)) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  for (const channel of ['msedge', 'chrome']) {
    try { browser = await chromium.launch({ channel }); break; } catch {}
  }
  assert.ok(browser, 'neither Edge nor Chrome could be launched');
  page = await browser.newPage();
  page.on('pageerror', (e) => problems.push(e.message));
  page.on('console', (m) => m.type() === 'error' && problems.push(m.text()));
  page.on('response', (r) => r.status() >= 400 && problems.push(`${r.status()} ${r.url()}`));
});
after(async () => {
  await browser?.close();
  spawn('taskkill', ['/pid', String(server.pid), '/t', '/f'], { stdio: 'ignore' }).on('error', () => server.kill());
  if (process.platform !== 'win32') server.kill();
});

const total = async () => {
  await page.waitForFunction(() => /^\d+ rules/.test(document.querySelector('#rule-search .count')?.textContent ?? ''));
  return parseInt(await page.locator('#rule-search .count').innerText());
};
const open = async (query) => { await page.goto(`${B}/rules/${query}`); return total(); };

test('a search term lists rules with working links', async () => {
  const all = await open('');
  assert.ok(all > 50);
  await page.fill('#rule-search input[type=search]', 'trailing');
  await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) < n, all);
  assert.ok(await total() > 0);
  const link = page.locator('#rule-search .results a').first();
  const title = await link.innerText();
  await link.click();
  await page.waitForURL(/\/rules\/[^/]+\/$/);
  assert.equal(await page.locator('h1').innerText(), title);
});

test('each filter narrows the results', async () => {
  const all = await open('');
  for (const name of ['language', 'category', 'fix', 'preset', 'upstream']) {
    const group = page.locator('#rule-search details', { hasText: new RegExp(`^${name}`) });
    const options = await group.locator('label').allTextContents();
    const smallest = options.map((t) => /^\s*(.*?)\s*\((\d+)\)\s*$/.exec(t)).filter((m) => +m[2] > 0).sort((a, b) => a[2] - b[2])[0];
    assert.ok(+smallest[2] < all, `${name}: ${smallest[0]}`);
    await group.evaluate((d) => (d.open = true));
    await group.locator('label').filter({ hasText: smallest[0].trim() }).locator('input').check();
    await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) < n, all);
    assert.equal(await total(), +smallest[2], name);
    await page.locator('#rule-search input[type=checkbox]:checked').uncheck();
    await page.waitForFunction((n) => parseInt(document.querySelector('#rule-search .count').textContent) === n, all);
  }
});

test('query parameters preselect', async () => {
  const all = await open('');
  assert.ok(await open('?language=csharp') < all);
  await page.fill('#rule-search input[type=search]', 'comment-space'); // a rule for all languages
  await page.waitForSelector('#rule-search .results a[href$="/rules/comment-space/"]');
  await page.waitForSelector('#rule-search input[type=checkbox]:checked');
  assert.ok(await open('?q=SA1005') > 0);
  assert.equal(await page.inputValue('#rule-search input[type=search]'), 'SA1005');
});

test('the sidebar has one Rules entry and there is no level filter', async () => {
  await open('');
  const links = page.locator('nav.sidebar a[href*="/rules"]');
  assert.equal(await links.count(), 1);
  assert.equal(await links.first().getAttribute('aria-current'), 'page');
  assert.equal(await page.locator('#rule-search details', { hasText: /^level/ }).count(), 0);
});

test('rule page shows squiggles, its example and configuration; an upstream id redirects', async () => {
  await page.goto(`${B}/rules/key-name-casing/`);
  assert.ok(await page.locator('.example pre').count() >= 1);
  assert.equal(await page.locator('.example .squiggle').count(), 3);
  assert.equal(await page.locator('.example .squiggle').first().getAttribute('title'), 'key-name-casing');
  assert.equal(await page.locator('.example pre', { hasText: '^^^' }).count(), 0);
  assert.equal(await page.locator('.meta', { hasText: 'level' }).count(), 0);
  assert.equal(await page.getByRole('heading', { name: 'Configuration' }).count(), 1);
  assert.equal(await page.locator('main h2, main h3').filter({ hasText: /^(Enable|Disable|With options|Suppress in code|Options)$/ }).count(), 0);
  assert.equal(await page.getByText('Input; hover').count(), 0);
  assert.equal(await page.locator('pre', { hasText: 'sudsy-disable-next-line' }).count(), 0);
  assert.ok(await page.locator('pre', { hasText: 'level: warn  # off (default), warn, error' }).count() === 1);
  await page.goto(`${B}/rules/sorted-keys/`);
  assert.ok(await page.locator('pre', { hasText: '# off (default), warn, error' }).filter({ hasText: 'options:' }).count() === 1);
  assert.ok(await page.locator('main table').count() >= 1);
  await page.goto(`${B}/rules/comment-space/`);
  await page.goto(`${B}/rules/sa1005/`);
  await page.waitForURL(`${B}/rules/comment-space/`);
});

test('the header search finds rules', async () => {
  await page.goto(`${B}/getting-started/`);
  await page.locator('button[data-open-modal]').first().click();
  await page.locator('dialog[open] input').fill('comment-space');
  await page.locator('dialog[open] a[href*="/rules/comment-space/"]').first().waitFor();
});

test('no console or network errors', () => assert.deepEqual(problems, []));
