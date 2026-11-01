/**
 * What the download links and cards do on the page.
 *
 * Every link starts out at the latest release's page, which works without this script. Once GitHub
 * names the latest release, a link whose `data-asset` names the end of a file's name points at that
 * file, a `data-file` shows the file's name, a `data-sha` links its checksum, and `data-version`
 * shows the release's version. The card for the visitor's device takes the amber button and a line
 * saying so, and the Home page's button follows it. A click on a card's file link opens that
 * platform's next steps beneath it, and focus moves to their heading.
 */
import { detect, type DeviceId, type UaData } from './detect';
import { latest, type Release } from './release';
import { formatMegabytes, readWords, say } from './words';

interface UaDataApi {
	platform?: string;
	getHighEntropyValues?(hints: string[]): Promise<UaData>;
}

async function uaData(): Promise<UaData | null> {
	const api = (navigator as Navigator & { userAgentData?: UaDataApi }).userAgentData;
	if (!api) return null;
	const low: UaData = { platform: api.platform };
	if (!api.getHighEntropyValues) return low;
	try {
		const high = await Promise.race([
			api.getHighEntropyValues(['platform', 'architecture', 'bitness']),
			new Promise<null>((resolve) => setTimeout(() => resolve(null), 400)),
		]);
		return high ? { ...low, ...high } : low;
	} catch {
		return low;
	}
}

/** The WebGL renderer's name, which says whether a Mac has Apple's chip. */
function gpu(): string | undefined {
	try {
		const gl = document.createElement('canvas').getContext('webgl');
		if (!gl) return undefined;
		const info = gl.getExtension('WEBGL_debug_renderer_info');
		const name = String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
		gl.getExtension('WEBGL_lose_context')?.loseContext();
		return name;
	} catch {
		return undefined;
	}
}

/** The card, the words' key and the other processor's file for each device. */
const DEVICES: Record<DeviceId, { card: string; key: string; suffix?: string }> = {
	mac: { card: 'mac', key: 'mac' },
	'mac-intel': { card: 'terminal', key: 'intel' },
	ios: { card: 'ios', key: 'ios' },
	android: { card: 'android', key: 'android' },
	'android-tv': { card: 'android-tv', key: 'tv' },
	'windows-x64': { card: 'windows', key: 'windows' },
	'windows-arm64': { card: 'windows', key: 'windows', suffix: '-windows-arm64-setup.exe' },
	'linux-x64': { card: 'linux', key: 'linux' },
	'linux-arm64': { card: 'linux', key: 'linux', suffix: '-linux-aarch64.AppImage' },
};

const find = (release: Release | null, suffix: string | undefined) => (suffix ? release?.assets.find((a) => a.name.endsWith(suffix)) : undefined);

function fillLinks(release: Release | null, version: string) {
	for (const el of document.body.querySelectorAll<HTMLElement>('[data-version]')) el.textContent = version;
	if (!release) return;
	for (const link of document.querySelectorAll<HTMLAnchorElement>('a[data-asset]')) {
		const asset = find(release, link.dataset.override ?? link.dataset.asset);
		if (!asset) continue;
		link.href = asset.browser_download_url;
		const size = link.querySelector<HTMLElement>('[data-asset-size]');
		if (size) size.textContent = formatMegabytes(asset.size);
	}
	for (const link of document.querySelectorAll<HTMLAnchorElement>('a[data-sha]')) {
		const asset = find(release, `${link.dataset.sha}.sha256`);
		if (asset) link.href = asset.browser_download_url;
	}
	for (const el of document.querySelectorAll<HTMLElement>('[data-file]')) {
		const asset = find(release, el.dataset.file);
		if (asset) el.textContent = asset.name;
	}
}

function openNext(link: HTMLElement) {
	const panel = document.querySelector<HTMLElement>(`[data-panel="${link.dataset.next}"]`);
	if (!panel) return;
	for (const other of document.querySelectorAll<HTMLElement>('.dl-next.open')) {
		if (other !== panel) {
			other.classList.remove('open');
			other.inert = true;
		}
	}
	panel.classList.add('open');
	panel.inert = false;
	const heading = panel.querySelector<HTMLElement>('h4');
	// Focus once the panel has room, so the page does not jump to a closed one.
	window.setTimeout(() => heading?.focus({ preventScroll: false }), 30);
}

async function run() {
	const w = readWords('dl.hero.');
	const fallback = document.documentElement.dataset.version ?? '';
	const data = await uaData();
	const ua = navigator.userAgent;
	const macLike = /Macintosh/.test(ua) && !data?.architecture;
	const device = detect(ua, data, { maxTouchPoints: navigator.maxTouchPoints, gpu: macLike ? gpu() : undefined });
	const info = device ? DEVICES[device] : null;

	for (const panel of document.querySelectorAll<HTMLElement>('.dl-next')) panel.inert = true;
	for (const link of document.querySelectorAll<HTMLElement>('a[data-next]')) link.addEventListener('click', () => openNext(link));

	if (info) {
		const card = document.querySelector<HTMLElement>(`[data-card="${info.card}"]`);
		const button = card?.querySelector<HTMLAnchorElement>('a.k-btn');
		if (button) {
			button.classList.add('k-btn-primary');
			if (info.suffix) button.dataset.override = info.suffix;
			card!.querySelector<HTMLElement>('.dl-suggested')!.hidden = false;
		}
	}

	const release = await Promise.race([latest(), new Promise<null>((resolve) => setTimeout(() => resolve(null), 1500))]);
	const version = release ? release.tag_name.replace(/^v/, '') : fallback;
	fillLinks(release, version);

	// The Home page's button and the line under it.
	const hero = document.querySelector<HTMLAnchorElement>('[data-hero-download]');
	const note = document.querySelector<HTMLElement>('[data-hero-note]');
	if (hero && note) {
		const key = info?.key ?? 'any';
		hero.textContent = say(w, `dl.hero.${key}.label`);
		note.textContent = say(w, `dl.hero.${key}.note`, { version });
		const card = info && document.querySelector<HTMLAnchorElement>(`[data-card="${info.card}"] a.k-btn`);
		if (card) hero.href = card.dataset.asset ? card.href : (card.dataset.steps ?? card.href);
	}
	if (!release) {
		const late = await latest();
		if (late) {
			fillLinks(late, late.tag_name.replace(/^v/, ''));
			const card = info && document.querySelector<HTMLAnchorElement>(`[data-card="${info.card}"] a.k-btn`);
			if (hero && card?.dataset.asset) hero.href = card.href;
		}
	}
}

run();
