/**
 * The latest release, as GitHub's API describes it, kept for ten minutes so a second page asks no
 * more and a release made since still shows soon. Null when the API fails or is rate-limited, and
 * the pages then keep the version and links they were built with.
 */
import { LATEST_API } from '../data/site';

export interface Release {
	tag_name: string;
	html_url: string;
	assets: { name: string; browser_download_url: string; size: number }[];
}

const KEY = 'kaeri-latest-release';
const KEEP_MS = 10 * 60 * 1000;

async function lookup(): Promise<Release | null> {
	try {
		const kept = JSON.parse(sessionStorage.getItem(KEY) ?? 'null') as { at: number; release: Release } | null;
		if (kept?.release && Date.now() - kept.at < KEEP_MS) return kept.release;
	} catch {
		// No storage in this window, or an older entry; ask GitHub.
	}
	try {
		const response = await fetch(LATEST_API, { headers: { Accept: 'application/vnd.github+json' } });
		if (!response.ok) return null;
		const release = (await response.json()) as Release;
		try {
			sessionStorage.setItem(KEY, JSON.stringify({ at: Date.now(), release }));
		} catch {
			// Kept for this page only.
		}
		return release;
	} catch {
		return null;
	}
}

/** The latest release; both scripts on a page share one request. */
let pending: Promise<Release | null> | undefined;
export const latest = () => (pending ??= lookup());
