// Kaeri's Cast receiver, on Google's Cast Application Framework. It plays what the sender loads
// on its own video element and draws Kaeri's screens over it: Ready with its city at dusk,
// Starting, the overlay, Paused, Waiting for the stream, Up next, Skipped, Waiting for the device,
// Couldn't play, Finished and the closing line. It never fetches media of its own; the sender's
// relay or the source serves every byte.

import { City, CLOSING, drift } from './city';
import { classify, codecLabel, refusedOf, type Report } from './errors';
import { loadStrings, type Strings } from './i18n';
import { startPlayer, type ErrorEventLike, type Player } from './player';
import {
	asDevice,
	deviceFromAgent,
	message,
	readExtras,
	typesetFiles,
	type Device,
	type LoadExtras,
} from './protocol';
import { Formats } from './formats/receiver';
import { DebugStatusLine, debugOn, devLog, forwardErrors } from './debug';
import { ShownFrames, SilentPicture } from './silent';
import { Typeset, canTypeset, type Failure } from './typeset';

declare const __BUILD__: string;

/** Board timings, in ms. */
const T = {
	starting: 600,
	spinner: 600,
	count: 4000,
	overlay: 4000,
	skipped: 3000,
	plain: 4000,
	backToReady: 10_000,
	close: 600_000,
	/** The page fades to black this long before it closes. */
	black: 400,
	/** Paused this long, the overlay starts drifting and the dim deepens. */
	pausedDrift: 300_000,
	/** The launch's own timings: the name follows the sky, the windows light one by one. */
	launch: 2500,
	hello: 1500,
	fatal: 500,
};

type Phase = 'idle' | 'loading' | 'playing' | 'gone' | 'error' | 'finished';

const base = new URL('./', location.href).href;
const now = () => performance.now();
const $ = <E extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as E;

const S = {
	phase: 'idle' as Phase,
	since: now(),
	/** The phase before, shown for the first 600 ms of a load from Ready. */
	prev: 'idle' as Phase,
	prevSince: now(),
	paused: false,
	pausedAt: 0,
	bufferingAt: null as number | null,
	overlayUntil: 0,
	upNext: null as null | { number?: string | number; title?: string; still?: string; startedAt: number; endsAt: number },
	skipped: null as null | { part: 'opening' | 'ending'; until: number },
	plainUntil: 0,
	plainSaid: false,
	plainSubs: false,
	media: { show: '', line: '', art: '', still: '', episode: null as string | null, url: '', type: '' },
	extras: {} as LoadExtras,
	device: 'phone' as Device,
	deviceKnown: false,
	startedAt: now(),
	goneAt: 0,
	error: null as Report | null,
	tracksEdited: false,
	fatalSent: false,
	language: navigator.language as string | null,
	/** Reduce Motion, from the sender's hello or the page's own setting. */
	still: typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches,
	/** The city has shown once; only the first Ready lights its windows one by one. */
	launched: false,
	closing: false,
};

let strings: Strings | null = null;
let player: Player | null = null;
const video = $<HTMLVideoElement>('video');
const typeset = new Typeset(video, base);
const silent = new SilentPicture();
const city = new City($('city'), now, new URL('city.js', base).href);
let shown: ShownFrames | null = null;
let debug: DebugStatusLine | null = null;
// Files and playlists unpacked on the receiver; their errors count like the player's own.
const formats = new Formats(video, base, () => (canTypeset() && !S.plainSubs ? typeset : null), (r) => unpackError(r));

const t = (id: string, args?: Record<string, string | number>) => strings?.t(id, args) ?? '';

function setPhase(phase: Phase) {
	if (S.phase === phase) return;
	S.prev = S.phase;
	S.prevSince = S.since;
	S.phase = phase;
	S.since = now();
	if (phase === 'loading') setTimeout(render, T.starting + 20);
	render();
}

// ---------- Messages to the sender ----------

function send(body: Record<string, unknown>, to?: string) {
	player?.send(body, to);
}

function ready(to?: string) {
	send(
		message('ready', {
			build: __BUILD__,
			caps: {
				typeset: canTypeset(),
				audioSwitch: true,
				// The receiver tries every stream; the sender plans from real failures only.
				codecs: [],
				screen: [Math.round(screen.width * devicePixelRatio), Math.round(screen.height * devicePixelRatio)],
			},
		}),
		to,
	);
}

function position(): number {
	return Math.round((player?.position() ?? 0) * 10) / 10;
}

function sendError(r: Report) {
	send(
		message('error', {
			part: r.part,
			// Whole, as the receiver names it (`hev1.1.6.L93.B0`, `ec-3`) or the engine's own word
			// (`video:h.264 high 10`); left out when unknown.
			...(r.codec ? { codec: r.codec } : {}),
			detail: r.detail,
			fetched: r.fetched,
			status: r.status,
			detailed: r.detailed,
			shaka: r.shaka,
			position: position(),
		}),
	);
}

// ---------- Strings ----------

async function useLanguage(wanted: string | null | undefined) {
	if (!wanted || (strings && wanted === S.language)) return;
	S.language = wanted;
	try {
		strings = await loadStrings(base, wanted);
		document.documentElement.lang = strings.tag;
	} catch (e) {
		console.warn('strings', e);
	}
	render();
}

function useDevice(value: unknown) {
	const d = asDevice(value);
	if (!d) return;
	S.device = d;
	S.deviceKnown = true;
	render();
}

// ---------- Drawing ----------

function on(id: string, shown: boolean) {
	$(id).classList.toggle('on', shown);
}
function text(id: string, value: string) {
	const el = $(id);
	if (el.textContent !== value) el.textContent = value;
}
function img(sel: string, src: string) {
	const el = document.querySelector(sel) as HTMLImageElement;
	if (src) {
		if (el.getAttribute('src') !== src) el.src = src;
	} else el.removeAttribute('src');
}

function clock(seconds: number) {
	const s = Math.max(0, Math.floor(seconds));
	const h = Math.floor(s / 3600);
	const m = Math.floor((s % 3600) / 60);
	const ss = String(s % 60).padStart(2, '0');
	return h ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

function episodeOf(extras: LoadExtras, metadata: Record<string, unknown> | undefined) {
	const n = extras.episode ?? metadata?.episode ?? metadata?.episodeNumber;
	return n == null || n === '' ? null : String(n);
}

/** Typeset subtitles draw, or a WebVTT track shows. */
function subtitlesShow(): boolean {
	if (typeset.active) return true;
	try {
		return [...video.textTracks].some((tr) => tr.mode === 'showing');
	} catch {
		return false;
	}
}

/** "Episode 3 · The Ferry at Dawn", as the overlay and Up next write it. */
function episodeLine(episode: string | number | null | undefined, title: string | undefined) {
	if (episode != null && title) return t('receiver-up-next-titled', { episode: String(episode), title });
	if (episode != null) return t('receiver-up-next-numbered', { episode: String(episode) });
	return title ?? '';
}

function place(el: HTMLElement, seconds: number | null) {
	const v = seconds == null ? '' : `translate(${drift(seconds).join('px, ')}px)`;
	if (el.style.transform !== v) el.style.transform = v;
}

/** The rows a plain cue takes on the TV, counting a wrap after about 64 characters. */
function rows(text: string): number {
	return text.split('\n').reduce((n, l) => n + Math.max(1, Math.ceil(l.replace(/<[^>]*>/g, '').length / 64)), 0);
}

/** Plain subtitles (WebVTT, SRT, the receiver's own text) lift above the overlay's band. */
const lifted = new WeakSet<TextTrackCue>();
let lift = false;
function liftSubtitles(up: boolean) {
	if (up === lift && !up) return;
	lift = up;
	let tracks: TextTrack[] = [];
	try {
		tracks = [...video.textTracks].filter((tr) => tr.mode === 'showing');
	} catch {
		return;
	}
	for (const tr of tracks) {
		const cues = up ? tr.activeCues : tr.cues;
		if (!cues) continue;
		for (const c of [...cues]) {
			if (!(c instanceof VTTCue)) continue;
			if (up && !lifted.has(c) && c.line === 'auto') {
				lifted.add(c);
				// Chromium places the cue's top edge at the line and ignores lineAlign, so the cue
				// starts one line height (5 %) per row above the band's top at 67 %.
				c.snapToLines = false;
				c.line = 64 - 5 * rows(c.text);
			} else if (!up && lifted.has(c)) {
				lifted.delete(c);
				c.snapToLines = true;
				c.line = 'auto';
			}
		}
	}
}

/** A line naming the codec this TV can't play, when the receiver reported one it can name. */
function reasonLine(r: Report | null): string {
	if (!r || (r.part !== 'video' && r.part !== 'audio')) return '';
	const name = codecLabel(r.codec);
	return name ? t(`receiver-error-${r.part}-codec`, { codec: name }) : '';
}

function render() {
	const at = now();
	// A load from Ready keeps Ready for 600 ms; most loads that fail fast fail in that time.
	const hold = S.phase === 'loading' && S.prev !== 'playing' && S.prev !== 'loading' && at - S.since < T.starting;
	const p = hold ? S.prev : S.phase;
	const since = hold ? S.prevSince : S.since;
	const ui = $('ui');
	ui.dataset.phase = S.phase;

	const playing = p === 'playing';
	const buffering = playing && !S.paused && S.bufferingAt != null;
	const spin = buffering && at - S.bufferingAt! >= T.spinner;
	const count = buffering && at - S.bufferingAt! >= T.count;
	const pausedLong = playing && S.paused && at - S.pausedAt >= T.pausedDrift;

	$('stage').classList.toggle('on', p === 'loading' || p === 'playing' || p === 'gone');

	// Ready: the city at dusk, the name and one line; the closing line in the last 30 s.
	const idle = p === 'idle';
	const idleFor = (at - since) / 1000;
	if (idle) {
		city.show(idleFor, S.still, !S.launched);
		S.launched = true;
	} else city.hide();
	on('city', idle);
	on('ready', idle);
	if (idle) place($('ready'), idleFor);
	const closing = idle && idleFor >= CLOSING;
	text('ready-line', t(`receiver-ready-${S.device}`));
	$('ready-line').classList.toggle('on', !closing && (S.deviceKnown || at - S.startedAt >= T.hello));
	$('closing-line').classList.toggle('on', closing);
	// The closing line is set from the start of Ready, so its first showing has nothing new to
	// format or lay out.
	if (idle) text('closing-line', t(`receiver-closing-${S.device}`, { seconds: closing ? Math.max(0, Math.ceil(T.close / 1000 - idleFor)) : 30 }));

	// Ten minutes with nothing to play: the page fades to black, then closes.
	const resting = p === 'idle' || p === 'error' || p === 'gone';
	const restingFor = at - (p === 'gone' ? S.goneAt : since);
	const black = resting && !hold && restingFor >= T.close - T.black;
	on('black', black);
	if (black && !S.closing) {
		S.closing = true;
		setTimeout(() => player?.close(), Math.max(0, T.close - restingFor));
	}

	// Art behind Starting and Couldn't play
	const art = S.media.art || S.media.still;
	img('#art img', art);
	on('art', !!art && (p === 'loading' || p === 'error'));
	ui.style.setProperty('--art', p === 'error' ? '0.25' : '0.4');

	// Dim: paused 45 %, deeper after 5 min; waiting 25 %; the device gone and errors 70 %.
	const dim = p === 'gone' || p === 'error' ? 0.7 : playing && S.paused ? (pausedLong ? 0.62 : 0.45) : spin ? 0.25 : 0;
	on('dim', dim > 0);
	if (dim) ui.style.setProperty('--dim', String(dim));

	// Overlay, or Starting while loading
	const overlay = p === 'loading' || (playing && (S.paused || at < S.overlayUntil));
	on('ov', overlay);
	text('ov-show', S.media.show);
	const line = episodeLine(S.media.episode, S.media.line);
	text('ov-episode', line);
	$('ov-episode').hidden = !line;
	text('starting-text', t('receiver-starting'));
	$('starting').classList.toggle('on', p === 'loading');
	if (playing) {
		const pos = player?.position() ?? 0;
		const dur = player?.duration() ?? 0;
		const bar = $('ov-bar').querySelector('i') as HTMLElement;
		const v = `scaleX(${dur > 0 ? Math.min(1, pos / dur).toFixed(4) : 0})`;
		if (bar.style.transform !== v) bar.style.transform = v;
		text('ov-at', clock(pos));
		text('ov-left', dur > 0 ? `−${clock(dur - pos)}` : '');
	}
	// Paused for 5 min, the overlay drifts too. It holds where it is until it fades.
	const ovText = $('ov').querySelector('.ov') as HTMLElement;
	if (pausedLong) {
		const d = (at - S.pausedAt - T.pausedDrift) / 1000;
		place(ovText, d);
		place($('paused'), d);
	} else if (!overlay) {
		place(ovText, null);
		place($('paused'), null);
	}
	liftSubtitles(playing && overlay);

	on('paused', playing && S.paused);
	text('paused-text', t('receiver-paused'));

	// Waiting for the stream: the lights after 600 ms, the count after 4 s.
	on('waiting', spin);
	$('waiting-text').classList.toggle('on', count);
	if (count) text('waiting-text', t('receiver-waiting', { seconds: Math.floor((at - S.bufferingAt!) / 1000) }));

	// Up next. While subtitles show, the card sits above them instead of over them.
	const next = playing ? S.upNext : null;
	on('upnext', !!next);
	$('upnext').classList.toggle('over-subs', !!next && subtitlesShow());
	$('upnext').classList.toggle('over-band', !!next && overlay);
	if (next) {
		img('#upnext img', next.still ?? '');
		text('upnext-count', t('receiver-up-next', { seconds: Math.max(0, Math.ceil((next.endsAt - at) / 1000)) }));
		text('upnext-title', episodeLine(next.number, next.title));
		countDown(next, at);
	}

	const skipped = playing && S.skipped && at < S.skipped.until ? S.skipped : null;
	on('skipped', !!skipped);
	if (skipped) text('skipped-text', t(`receiver-skipped-${skipped.part}`));

	on('plain', playing && at < S.plainUntil);
	text('plain', t('receiver-plain-subs'));

	// The device stopped answering
	on('gone', p === 'gone');
	if (p === 'gone') place($('gone'), (at - S.goneAt) / 1000);
	text('gone-title', t(`receiver-sender-gone-${S.device}`));
	text('gone-hint', t(`receiver-sender-gone-hint-${S.device}`));

	// Couldn't play
	on('error', p === 'error');
	if (p === 'error') place($('error'), (at - since) / 1000);
	text('error-title', S.media.episode ? t('receiver-error', { episode: S.media.episode }) : t('receiver-error-unnumbered'));
	text('error-show', S.media.show);
	text('error-reason', reasonLine(S.error));
	text('error-hint', t(`receiver-error-hint-${S.device}`));

	// Finished
	on('finished', p === 'finished');
	img('#finished img', S.media.still || S.media.art);
	text('finished-show', S.media.show);
	text(
		'finished-line',
		S.media.episode ? t('receiver-finished', { episode: S.media.episode }) : t('receiver-finished-unnumbered'),
	);
}

/** The amber line under Up next's still fills as it counts down, on the compositor. */
function countDown(next: { startedAt: number; endsAt: number }, at: number) {
	const fill = $('upnext').querySelector('.track i') as HTMLElement;
	const key = `${next.startedAt}:${next.endsAt}`;
	if (fill.dataset.key === key) return;
	fill.dataset.key = key;
	const span = Math.max(1, next.endsAt - next.startedAt);
	fill.style.transition = 'none';
	fill.style.transform = `scaleX(${Math.min(1, Math.max(0, (at - next.startedAt) / span)).toFixed(4)})`;
	void fill.offsetWidth;
	fill.style.transition = `transform ${Math.max(0, next.endsAt - at)}ms linear`;
	fill.style.transform = 'scaleX(1)';
}

function fit() {
	const s = Math.min(innerWidth / 1280, innerHeight / 720);
	const ui = $('ui');
	ui.style.setProperty('--s', String(s));
	ui.style.setProperty('--x', `${(innerWidth - 1280 * s) / 2}px`);
	ui.style.setProperty('--y', `${(innerHeight - 720 * s) / 2}px`);
}

/** The last picture, held under the waiting card while the episode can't go on. */
function holdPicture() {
	const still = $<HTMLCanvasElement>('still');
	try {
		if (!video.videoWidth) return;
		still.width = video.videoWidth;
		still.height = video.videoHeight;
		still.getContext('2d')?.drawImage(video, 0, 0);
		still.classList.add('on');
	} catch {
		/* no picture to hold */
	}
}
function releasePicture() {
	$('still').classList.remove('on');
}

// ---------- Typeset subtitles ----------

function plainSubtitles(reason: 'slow' | Failure) {
	devLog(`plain subtitles: ${reason}`);
	S.plainSubs = true;
	document.documentElement.classList.remove('typeset');
	void typeset.hide();
	formats.plainText();
	if (reason === 'slow' || reason === 'renderer') {
		if (!S.plainSaid) {
			S.plainSaid = true;
			S.plainUntil = now() + T.plain;
		}
		send(message('subtitles-plain', { reason }));
	} else {
		// A subtitle or font file didn't load: plain subtitles, and Messages records it.
		sendError({
			part: 'subtitles',
			codec: null,
			detail: `typeset ${reason} didn't load`,
			fetched: reason,
			status: null,
			detailed: null,
			shaka: null,
			lost: false,
		});
	}
	render();
}

async function applyTypeset() {
	let active = player?.activeTextTracks() ?? [];
	// Until the sender changes tracks, the load's own pick counts when CAF hasn't turned it on.
	if (!S.tracksEdited && !active.length && S.extras.subtitle != null) active = [S.extras.subtitle];
	// A track inside the file, or a PGS file: the receiver draws it.
	const entry = (S.extras.subtitles ?? []).find((s) => active.includes(s.id)) ?? null;
	if (await formats.showSubtitle(entry)) {
		document.documentElement.classList.remove('typeset');
		return;
	}
	const twin = typesetFiles(S.extras).find((s) => active.includes(s.trackId));
	if (!twin || S.plainSubs || !canTypeset()) {
		document.documentElement.classList.remove('typeset');
		await typeset.hide();
		return;
	}
	document.documentElement.classList.add('typeset');
	try {
		await typeset.show(twin.ass, twin.fonts);
	} catch (reason) {
		plainSubtitles(reason as Failure);
	}
}

// ---------- Errors ----------

/** After a failure only: the load's codecs this TV's MediaSource says no to, video first. */
function refused(): string[] {
	if (typeof MediaSource === 'undefined') return [];
	return refusedOf([S.extras.video?.codec, S.extras.audio?.codec], (type) => MediaSource.isTypeSupported(type));
}

/** The episode can't go on: tell the sender why, then show it. */
function fatal(report: Report) {
	if (S.fatalSent) return;
	S.fatalSent = true;
	sendError(report);
	void typeset.hide();
	if (report.lost && (S.phase === 'playing' || S.phase === 'gone')) {
		// The relay or the source stopped answering mid-episode: hold the picture and wait for
		// the sender, which loads the episode again where it stopped.
		holdPicture();
		if (S.phase !== 'gone') S.goneAt = now();
		setPhase('gone');
	} else {
		setPhase('error');
	}
}

/** An error from the receiver's own unpacking. Subtitles keep playing; the rest ends the episode. */
function unpackError(report: Report) {
	console.warn('unpacking', report.part, report.detail);
	if (report.part === 'subtitles') {
		sendError(report);
		return;
	}
	S.error = report;
	fatal(report);
}

/** Whether this load has a picture to watch for: the file has a video track, or the sender says so. */
function hasVideo(): boolean {
	const u = formats.current;
	if (u?.tracks.some((t) => t.kind === 'video')) return true;
	if (S.extras.video != null) return true;
	return !u && /^video\//i.test(S.media.type);
}

function silentPicture() {
	sendError({
		part: 'video',
		codec: S.extras.video?.codec ?? null,
		detail: `sound without picture: no decoded video frame for 3 s (videoWidth ${video.videoWidth})`,
		fetched: null,
		status: null,
		detailed: null,
		shaka: null,
		lost: false,
	});
}

// ---------- The player ----------

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function onLoad(request: any) {
	const media = request?.media ?? {};
	const md = media.metadata ?? {};
	const extras = readExtras(request);
	S.extras = extras;
	S.media = {
		show: md.seriesTitle || md.title || '',
		line: md.seriesTitle ? (md.title ?? '') : '',
		art: md.images?.[0]?.url ?? '',
		still: extras.still ?? '',
		episode: episodeOf(extras, md),
		url: media.contentUrl || media.contentId || '',
		type: media.contentType || '',
	};
	S.paused = false;
	S.bufferingAt = null;
	S.upNext = null;
	S.skipped = null;
	S.error = null;
	S.fatalSent = false;
	S.tracksEdited = false;
	S.plainSubs = !!extras.plainSubtitles;
	silent.reset();
	releasePicture();
	document.documentElement.classList.remove('typeset');
	void typeset.hide();
	if (extras.language) void useLanguage(extras.language);
	if (extras.device) useDevice(extras.device);
	S.phase = 'idle';
	setPhase('loading');
	return request;
}

function onPlaying() {
	S.bufferingAt = null;
	if (S.phase === 'loading' || S.phase === 'gone') {
		releasePicture();
		S.overlayUntil = now() + T.overlay;
		S.paused = false;
		setPhase('playing');
		void applyTypeset();
	} else if (S.paused) {
		S.paused = false;
		S.overlayUntil = now() + T.overlay;
	}
	render();
}

function onFinished(reason: 'end' | 'stopped' | 'error' | 'interrupted' | 'other') {
	if (reason === 'error') {
		fatal(S.error ?? classify({}, player?.errorNames ?? {}, refused()));
		return;
	}
	if (reason === 'interrupted') return;
	const pos = player?.position() ?? 0;
	const dur = player?.duration() ?? 0;
	void typeset.hide();
	document.documentElement.classList.remove('typeset');
	const atEnd = reason === 'end' || (dur > 0 && pos >= dur - 30);
	if (atEnd && S.upNext) return; // the sender loads the next episode
	setPhase(atEnd ? 'finished' : 'idle');
}

function onSenderMessage(data: unknown, senderId: string) {
	if (!data || typeof data !== 'object') return;
	const m = data as Record<string, unknown>;
	switch (m.type) {
		case 'hello': {
			if (m.motion === 'reduce' || m.motion === 'full') {
				S.still = m.motion === 'reduce';
				document.documentElement.classList.toggle('still', S.still);
			}
			useDevice(m.device);
			void useLanguage(typeof m.language === 'string' ? m.language : null);
			ready(senderId);
			if (S.phase === 'gone') {
				send(message('sender-gone', { waited: Math.round((now() - S.goneAt) / 1000) }), senderId);
				releasePicture();
				// The sender resumes, or loads again where the episode stopped.
				setPhase(S.fatalSent ? 'idle' : 'playing');
			}
			break;
		}
		case 'sender-away': {
			useDevice(m.device);
			if (S.phase === 'playing' || S.phase === 'loading') {
				player?.pause();
				holdPicture();
				S.goneAt = now();
				setPhase('gone');
			}
			break;
		}
		case 'up-next': {
			const seconds = Number(m.seconds);
			S.upNext = Number.isFinite(seconds) && m.seconds != null
				? {
						number: m.number as string | number | undefined,
						title: m.title as string | undefined,
						still: m.still as string | undefined,
						startedAt: now(),
						endsAt: now() + seconds * 1000,
					}
				: null;
			render();
			break;
		}
		case 'skipped': {
			if (m.part === 'opening' || m.part === 'ending') S.skipped = { part: m.part, until: now() + T.skipped };
			render();
			break;
		}
		case 'audio': {
			if (typeof m.language === 'string')
				player?.switchAudio(m.language, typeof m.name === 'string' ? m.name : undefined);
			break;
		}
		default:
			break; // unknown types are ignored
	}
}

function onError(e: ErrorEventLike, isFatal: () => boolean) {
	const report = classify(e, player?.errorNames ?? {}, refused());
	console.warn('player error', report.detail);
	if (report.part === 'subtitles') {
		sendError(report); // the episode plays on without them
		return;
	}
	S.error = report;
	// A failed load is told at once, before CAF's LOAD_FAILED reaches the sender.
	if (S.phase === 'loading') return fatal(report);
	// Mid-episode, a fatal error leaves the player idle; a recovered one doesn't.
	setTimeout(() => {
		if (isFatal()) fatal(report);
	}, T.fatal);
}

/** The TV remote while Up next counts: select plays it now, back cancels it. */
function onKey(e: KeyboardEvent) {
	if (S.phase !== 'playing' || !S.upNext) return;
	const action = e.key === 'Enter' || e.key === 'MediaTrackNext' ? 'play' : e.key === 'Escape' || e.key === 'GoBack' || e.key === 'BrowserBack' ? 'cancel' : null;
	if (!action) return;
	e.preventDefault();
	send(message('up-next', { action }));
	if (action === 'cancel') {
		S.upNext = null;
		render();
	}
}

// ---------- Once a second ----------

function everySecond() {
	const at = now();
	const playing = S.phase === 'playing' && !S.paused && S.bufferingAt == null && !video.seeking;
	if (typeset.active && typeset.tick(playing)) plainSubtitles('slow');
	if ((S.phase === 'playing' || S.phase === 'loading') && hasVideo()) {
		// The element's own clock, not CAF's state: with no picture CAF can stay in BUFFERING
		// while the sound plays.
		const quality = typeof video.getVideoPlaybackQuality === 'function' ? video.getVideoPlaybackQuality() : null;
		if (
			silent.check({
				time: video.currentTime,
				videoWidth: video.videoWidth,
				frames: quality ? quality.totalVideoFrames - quality.droppedVideoFrames : null,
				presented: shown?.count ?? null,
				playing: !video.paused && !video.seeking && !video.ended,
			})
		)
			void silentPicture();
	}
	if (S.phase === 'finished' && at - S.since >= T.backToReady) setPhase('idle');
	const resting = S.phase === 'idle' || S.phase === 'error' || S.phase === 'gone';
	if (resting && !S.closing && at - (S.phase === 'gone' ? S.goneAt : S.since) >= T.close) {
		// Ten minutes with nothing to play: the TV goes back to its own screen.
		player?.close();
	}
	if (!resting) S.closing = false;
}

// ---------- Start ----------

async function start() {
	fit();
	document.documentElement.classList.add('launch');
	document.documentElement.classList.toggle('still', S.still);
	setTimeout(() => document.documentElement.classList.remove('launch'), T.launch);
	shown = new ShownFrames(video);
	if (debugOn()) {
		forwardErrors();
		debug = new DebugStatusLine(video, formats, () => shown?.count ?? null);
		(window as unknown as Record<string, unknown>).__kaeriDebug = () => debug?.status ?? null;
	}
	addEventListener('resize', fit);
	addEventListener('keydown', onKey);
	player = startPlayer(
		video,
		{
		load: onLoad,
		playing: onPlaying,
		paused: () => {
			if (S.phase !== 'playing') return;
			if (!S.paused) S.pausedAt = now();
			S.paused = true;
			render();
		},
		buffering: (isBuffering) => {
			if (S.phase !== 'playing') return;
			S.bufferingAt = isBuffering ? (S.bufferingAt ?? now()) : null;
			render();
		},
		seeked: () => {
			S.overlayUntil = now() + T.overlay;
			render();
		},
		finished: onFinished,
		error: onError,
		tracksChanged: () => {
			S.tracksEdited = true;
			void applyTypeset();
		},
		senderMessage: onSenderMessage,
		senderConnected: (senderId, userAgent) => {
			if (!S.deviceKnown) S.device = deviceFromAgent(userAgent);
			ready(senderId);
		},
		},
		formats,
	);

	render(); // the sky first, while the strings load
	await useLanguage(S.language);
	setInterval(render, 250);
	setInterval(everySecond, 1000);
	render();
	// A sender that connected before the listener heard it still gets `ready`.
	ready();

	if ((window as unknown as { __cafStub?: boolean }).__cafStub)
		(window as unknown as Record<string, unknown>).__receiver = { S, render, typeset, plainSubtitles, silent, formats, city, get debug() { return debug?.status ?? null; } };
}

void start();
