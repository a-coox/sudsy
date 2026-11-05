// Shared by astro.config.mjs and scripts/generate.mjs.
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

export const site = 'https://a-coox.github.io';
export const base = '/sudsy';
export const rulesPath = process.env.SUDSY_RULES_JSON ?? fileURLToPath(new URL('../docs/rules/rules.json', import.meta.url));
export const rules = () => JSON.parse(readFileSync(rulesPath, 'utf8')).rules;
