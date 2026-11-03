/** Build-time only: the page's server code reads the version, a client script must not import this. */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

import { LATEST_API } from './site';

/** The workspace's version, which main carries ahead of the stable release while a beta is out. */
function workspace(): string {
	try {
		// The build runs in website/, and the bundle moves this module, so the path is from there.
		const toml = readFileSync(join(process.cwd(), '..', 'Cargo.toml'), 'utf8');
		return toml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? '0.1.0';
	} catch {
		return '0.1.0';
	}
}

/** The latest stable release's version, asked of GitHub once per build. */
async function stable(): Promise<string | undefined> {
	const token = process.env.GITHUB_TOKEN;
	try {
		const response = await fetch(LATEST_API, {
			headers: { Accept: 'application/vnd.github+json', ...(token ? { Authorization: `Bearer ${token}` } : {}) },
			signal: AbortSignal.timeout(10_000),
		});
		if (!response.ok) return undefined;
		const { tag_name } = (await response.json()) as { tag_name?: string };
		return tag_name?.replace(/^v/, '');
	} catch {
		return undefined;
	}
}

/**
 * The version the download page names: the latest stable release when GitHub answers the build,
 * else the workspace's. A script on the page asks GitHub again, so a release made since the
 * build shows too.
 */
export const VERSION: string = (await stable()) ?? workspace();
