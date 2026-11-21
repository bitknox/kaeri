// Checks of the built site's HTML, for what only the pages as built show. Build first:
//
//   bun run build && bun test scripts/built.test.ts
import { describe, expect, test } from 'bun:test';
import { existsSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const dist = join(import.meta.dir, '..', 'dist');

/** A built page's headings, h1 to h6, in the order they stand in the page. */
function headings(path: string): { level: number; text: string }[] {
	const file = join(dist, path, 'index.html');
	if (!existsSync(file)) throw new Error(`${file} is missing: build the site first (bun run build)`);
	const html = readFileSync(file, 'utf8');
	return [...html.matchAll(/<h([1-6])\b[^>]*>([\s\S]*?)<\/h\1>/g)].map((m) => ({
		level: Number(m[1]),
		text: m[2].replace(/<[^>]+>/g, '').replace(/\s+/g, ' ').trim(),
	}));
}

describe('the pages with the download cards', () => {
	// A screen reader's heading list, and the focus a file link moves to its panel's heading,
	// read the levels: each heading is at most one below the one before it.
	for (const path of ['', 'download']) {
		test(`/${path} skips no heading level`, () => {
			const skips: string[] = [];
			let previous = 0;
			for (const { level, text } of headings(path)) {
				if (level > previous + 1) skips.push(`h${level} "${text}" after h${previous}`);
				previous = level;
			}
			expect(skips).toEqual([]);
		});
	}
});
