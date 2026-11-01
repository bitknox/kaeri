/** Build-time only: the page's server code reads the workspace version, a client script must not import this. */
import { readFileSync } from 'node:fs';

/** The version the workspace is at, which the file names on the download page carry until GitHub names the latest. */
export const VERSION: string = (() => {
	try {
		const toml = readFileSync(new URL('../../../Cargo.toml', import.meta.url), 'utf8');
		return toml.match(/^version\s*=\s*"([^"]+)"/m)?.[1] ?? '0.1.0';
	} catch {
		return '0.1.0';
	}
})();
