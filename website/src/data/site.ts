/**
 * Links and facts the pages share. The release files are named by version (the apps' in
 * .github/workflows/apps-release.yml, the terminal's in release.yml), so a page links to the latest
 * release and src/scripts/download.ts swaps in each file's own link once GitHub says which it is.
 */

export const GITHUB = 'https://github.com/bitknox/kaeri';
export const RELEASES = `${GITHUB}/releases`;
export const LATEST = `${GITHUB}/releases/latest`;
export const LATEST_API = 'https://api.github.com/repos/bitknox/kaeri/releases/latest';
export const SIDESTORE_SOURCE = `${LATEST}/download/sidestore.json`;

export const INSTALL_GUIDE = '/docs/getting-started/installation/';
export const FEATURES = '/features/';
export const DOWNLOAD = '/download/';
export const STORES = '/docs/plugins/stores/';
export const ADDING_BOOKS = '/docs/reading/adding-books/';
export const PRIVACY = '/docs/privacy/';
export const LEGAL = '/legal/';

/** The demo work's two parts on Wikimedia Commons, from crates/kaeri-core/src/demo.rs. */
export const DEMO_PART_1 = 'https://commons.wikimedia.org/wiki/File:%E3%81%93%E3%81%86%E3%81%97%E3%81%99%EF%BC%81_-3.1%E3%80%8C%E3%82%BB%E3%82%AD%E3%83%A5%E3%83%AA%E3%83%86%E3%82%A3%E3%81%AB%E5%AE%8C%E7%92%A7%E3%82%92%E6%B1%82%E3%82%81%E3%82%8B%E3%81%AE%E3%81%AF%E9%96%93%E9%81%95%E3%81%A3%E3%81%A6%E3%81%84%E3%82%8B%E3%81%A0%E3%82%8D%E3%81%86%E3%81%8B%E3%80%8D%28Part1-4%29.webm';
export const DEMO_PART_2 = 'https://commons.wikimedia.org/wiki/File:%E3%81%93%E3%81%86%E3%81%97%E3%81%99%EF%BC%81_-3.2%E3%80%8C%E3%82%BB%E3%82%AD%E3%83%A5%E3%83%AA%E3%83%86%E3%82%A3%E3%81%AB%E5%AE%8C%E7%92%A7%E3%82%92%E6%B1%82%E3%82%81%E3%82%8B%E3%81%AE%E3%81%AF%E9%96%93%E9%81%95%E3%81%A3%E3%81%A6%E3%81%84%E3%82%8B%E3%81%A0%E3%82%8D%E3%81%86%E3%81%8B_%28Part2-4%29%E3%80%8D.webm';
export const CC = 'https://creativecommons.org/licenses/by/4.0/';

/** One file of an app in the release. */
export interface AppFile {
	/** The file's name with the version left out, as the release names it. */
	suffix: string;
	/** The processor it is for, where an app has a file for each. */
	arch?: string;
}

export type AppId = 'mac' | 'ios' | 'android' | 'android-tv' | 'windows' | 'linux' | 'terminal';

export const FILES: Record<'mac' | 'android' | 'android-tv' | 'windows' | 'linux', AppFile[]> = {
	mac: [{ suffix: '-macos.dmg' }],
	android: [{ suffix: '-android-arm64.apk' }],
	'android-tv': [{ suffix: '-android-tv-armv7.apk', arch: 'armv7' }, { suffix: '-android-tv-arm64.apk', arch: 'arm64' }],
	windows: [{ suffix: '-windows-x64-setup.exe', arch: 'x64' }, { suffix: '-windows-arm64-setup.exe', arch: 'Arm64' }],
	linux: [{ suffix: '-linux-x86_64.AppImage', arch: 'x86-64' }, { suffix: '-linux-aarch64.AppImage', arch: 'ARM64' }],
};
