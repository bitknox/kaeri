import { expect, test } from 'bun:test';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

// Every page that says which Android the apps need, against the one floor every Android app and
// module builds with (KaeriSdk.MIN). A floor that moves without these pages tells people their
// device can't run Kaeri when it can, or the other way round.
const repo = join(new URL('.', import.meta.url).pathname, '../../..');
const read = (path: string) => readFileSync(join(repo, path), 'utf8');

/** The Android release that brought each API level the apps could take as their floor. */
const RELEASE: Record<string, string> = { '29': '10', '30': '11', '31': '12', '33': '13', '34': '14', '35': '15', '36': '16', '37': '17' };

/** "Android 11 or later", "Android TV or Google TV 11 or later", "Android 11 (API 30) and up". */
const CLAIM = /Android(?:\s+TV)?(?:\s+or\s+Google\s+TV)?\s+(\d+)(?:\s+\(API\s+(\d+)\))?\s+(?:or\s+later|and\s+up)/g;
/** "minSdk 30", as the Android notes put it. */
const SDK = /minSdk\s+(\d+)/g;

function under(dir: string, ext: RegExp): string[] {
	return readdirSync(join(repo, dir)).flatMap((name) => {
		const path = `${dir}/${name}`;
		if (statSync(join(repo, path)).isDirectory()) return under(path, ext);
		return ext.test(name) ? [path] : [];
	});
}

/** Pages that name the floor; each must keep naming it in a form this test reads. */
const NAMED = [
	'website/src/content/i18n/en.json',
	'README.md',
	'packaging/apps/release-notes.md',
	'packaging/README.md',
	'android/README.md',
	'android/CLAUDE.md',
	'docs/remote-protocol.md',
];
const PAGES = [...new Set([...under('website/src/content/i18n', /\.json$/), ...under('website/src/content/docs', /\.mdx?$/), ...NAMED])];

test('every page names the Android floor the apps build with', () => {
	const api = read('android/build-logic/src/main/kotlin/KaeriSdk.kt').match(/const val MIN = (\d+)/)?.[1];
	expect(api, 'KaeriSdk.MIN').toBeDefined();
	const release = RELEASE[api!];
	expect(release, `the Android release of API ${api}`).toBeDefined();

	const wrong: string[] = [];
	const named = new Set<string>();
	for (const page of PAGES) {
		const text = read(page);
		for (const m of text.matchAll(CLAIM)) {
			named.add(page);
			if (m[1] !== release || (m[2] !== undefined && m[2] !== api)) wrong.push(`${page}: ${m[0].replace(/\s+/g, ' ')}`);
		}
		for (const m of text.matchAll(SDK)) {
			named.add(page);
			if (m[1] !== api) wrong.push(`${page}: ${m[0]}`);
		}
	}
	expect(wrong, `the floor is Android ${release} (API ${api})`).toEqual([]);
	// A page reworded out of these patterns would leave this check without a word.
	for (const page of NAMED) expect(named.has(page), `${page} names the floor`).toBe(true);
	expect(named.has('website/src/content/docs/docs/getting-started/requirements.mdx')).toBe(true);
});
