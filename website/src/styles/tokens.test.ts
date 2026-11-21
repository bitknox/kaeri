import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { contrast, Palette, rules, sheet, THEMES } from './palette';

const website = new URL('../../', import.meta.url).pathname;

/** The site's sources that can read or set a custom property, with their text. */
function sources(dir: string): { path: string; text: string }[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return sources(path);
		return /\.(css|astro|ts|mjs|md|mdx)$/.test(name) && !name.endsWith('.test.ts') ? [{ path, text: readFileSync(path, 'utf8') }] : [];
	});
}
const files = [...sources(join(website, 'src')), ...['astro.config.mjs', 'ec.config.mjs'].map((name) => ({ path: join(website, name), text: readFileSync(join(website, name), 'utf8') }))];
const where = (path: string) => path.slice(website.length);

describe('the --ani- custom properties', () => {
	// A var() of a property that is never set, with no fallback, makes the whole declaration
	// invalid, and the browser drops it without a word.
	test('every one that is read without a fallback is defined', () => {
		const defined = new Set(files.flatMap(({ text }) => [...text.matchAll(/(--ani-[\w-]+)['"]?\s*:|setProperty\(\s*['"](--ani-[\w-]+)/g)].map((m) => m[1] ?? m[2])));
		const undefinedReads = files.flatMap(({ path, text }) =>
			[...text.matchAll(/var\(\s*(--ani-[\w-]+)\s*\)/g)].filter((m) => !defined.has(m[1])).map((m) => `${where(path)}: ${m[1]}`),
		);
		expect(undefinedReads).toEqual([]);
	});
});

describe('the docs asides', () => {
	const docs = rules(sheet('docs.css'));
	/** What docs.css gives a property in the rules for a selector, the last rule winning. */
	const declared = (selector: string, property: string) =>
		docs
			.filter((r) => !r.conditional && r.selectors.includes(selector))
			.flatMap((r) => r.declarations)
			.findLast(([p]) => p === property)?.[1];

	// Starlight's four kinds. Each aside shows its kind only by the colour of its mark (docs.css).
	for (const theme of THEMES) {
		test(`mark every kind in a colour that stands out from the aside, in the ${theme} theme`, () => {
			const palette = new Palette(theme, sheet('starlight-tokens.css'), sheet('site.css'), sheet('docs.css'));
			const aside = palette.resolve(declared('.sl-markdown-content .starlight-aside', 'background-color')!);
			for (const kind of ['note', 'tip', 'caution', 'danger']) {
				const mark = declared(`.starlight-aside--${kind}`, '--ani-aside-mark');
				expect(mark, `${kind} has no mark`).toBeDefined();
				expect(contrast(palette.resolve(mark!), aside), `${kind}'s mark on the aside`).toBeGreaterThanOrEqual(3);
			}
		});
	}
});
