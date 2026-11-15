// Kaeri's Cast receiver, on Google's Cast Application Framework. It plays what the sender loads
// on its own video element and draws Kaeri's screens over it: Ready, Starting, the overlay,
// Paused, Waiting for the stream, Up next, Skipped, Waiting for the device, Couldn't play and
// Finished. It never fetches media of its own; the sender's relay or the source serves every byte.

import { classify, refusedOf, type Report } from './errors';
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
import { SilentPicture } from './silent';
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
	paused: false,
	bufferingAt: null as number | null,
	overlayUntil: 0,
	upNext: null as null | { number?: string | number; title?: string; still?: string; endsAt: number },
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
};

let strings: Strings | null = null;
let player: Player | null = null;
const video = $<HTMLVideoElement>('video');
const typeset = new Typeset(video, base);
const silent = new SilentPicture();

const t = (id: string, args?: Record<string, string | number>) => strings?.t(id, args) ?? '';

function setPhase(phase: Phase) {
	if (S.phase === phase) return;
	S.phase = phase;
	S.since = now();
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
			// As the receiver names it (`hev1`, `ec-3`, or the engine's own word); left out when unknown.
			...(r.codec ? { codec: r.codec.split('.')[0] } : {}),
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

/** "Episode 3 · The Ferry at Dawn", as the overlay and Up next write it. */
function episodeLine(episode: string | number | null | undefined, title: string | undefined) {
	if (episode != null && title) return t('receiver-up-next-titled', { episode: String(episode), title });
	if (episode != null) return t('receiver-up-next-numbered', { episode: String(episode) });
	return title ?? '';
}

function render() {
	const p = S.phase;
	const at = now();
	const ui = $('ui');
	ui.dataset.phase = p;

	const playing = p === 'playing';
	const buffering = playing && !S.paused && S.bufferingAt != null;
	const spin = buffering && at - S.bufferingAt! >= T.spinner;
	const count = buffering && at - S.bufferingAt! >= T.count;

	$('stage').classList.toggle('on', p === 'loading' || p === 'playing' || p === 'gone');

	// Ready
	on('ready', p === 'idle');
	text('ready-line', t(`receiver-ready-${S.device}`));
	$('ready-line').classList.toggle('on', S.deviceKnown || at - S.startedAt >= T.hello);

	// Art behind Starting and Couldn't play
	const art = S.media.art || S.media.still;
	img('#art img', art);
	on('art', !!art && (p === 'loading' || p === 'error'));
	ui.style.setProperty('--art', p === 'error' ? '0.25' : '0.4');

	// Dim: paused 45%, waiting 25%, the device gone 62%
	const dim = p === 'gone' ? 0.62 : playing && S.paused ? 0.45 : spin ? 0.25 : 0;
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
	$('starting').classList.toggle('on', p === 'loading' && at - S.since >= T.starting);
	if (playing) {
		const pos = player?.position() ?? 0;
		const dur = player?.duration() ?? 0;
		const bar = $('ov-bar').firstElementChild as HTMLElement;
		bar.style.width = dur > 0 ? `${Math.min(100, (100 * pos) / dur).toFixed(2)}%` : '0';
		text('ov-at', clock(pos));
		text('ov-left', dur > 0 ? `−${clock(dur - pos)}` : '');
	}

	on('paused', playing && S.paused);
	text('paused-text', t('receiver-paused'));

	on('spin', spin);
	on('waiting', count);
	if (count) text('waiting', t('receiver-waiting', { seconds: Math.floor((at - S.bufferingAt!) / 1000) }));

	// Up next
	const next = playing ? S.upNext : null;
	on('upnext', !!next);
	if (next) {
		img('#upnext img', next.still ?? '');
		text('upnext-count', t('receiver-up-next', { seconds: Math.max(0, Math.ceil((next.endsAt - at) / 1000)) }));
		text('upnext-title', episodeLine(next.number, next.title));
	}

	const skipped = playing && S.skipped && at < S.skipped.until ? S.skipped : null;
	on('skipped', !!skipped);
	if (skipped) text('skipped-text', t(`receiver-skipped-${skipped.part}`));

	on('plain', playing && at < S.plainUntil);
	text('plain', t('receiver-plain-subs'));

	// The device stopped answering
	on('gone', p === 'gone');
	text('gone-title', t(`receiver-sender-gone-${S.device}`));
	text('gone-hint', t(`receiver-sender-gone-hint-${S.device}`));

	// Couldn't play
	on('error', p === 'error');
	text('error-title', S.media.episode ? t('receiver-error', { episode: S.media.episode }) : t('receiver-error-unnumbered'));
	text('error-show', S.media.show);
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
	S.plainSubs = true;
	document.documentElement.classList.remove('typeset');
	void typeset.hide();
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
	if (S.phase === 'playing') {
		const quality = typeof video.getVideoPlaybackQuality === 'function' ? video.getVideoPlaybackQuality() : null;
		if (
			silent.check({
				time: video.currentTime,
				videoWidth: video.videoWidth,
				frames: quality ? quality.totalVideoFrames : null,
				playing,
			})
		)
			void silentPicture();
	}
	if (S.phase === 'finished' && at - S.since >= T.backToReady) setPhase('idle');
	const resting = S.phase === 'idle' || S.phase === 'error' || S.phase === 'gone';
	if (resting && at - (S.phase === 'gone' ? S.goneAt : S.since) >= T.close) {
		// Ten minutes with nothing to play: the TV goes back to its own screen.
		player?.close();
	}
}

// ---------- Start ----------

async function start() {
	fit();
	addEventListener('resize', fit);
	addEventListener('keydown', onKey);
	player = startPlayer(video, {
		load: onLoad,
		playing: onPlaying,
		paused: () => {
			if (S.phase !== 'playing') return;
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
	});

	await useLanguage(S.language);
	setInterval(render, 250);
	setInterval(everySecond, 1000);
	render();
	// A sender that connected before the listener heard it still gets `ready`.
	ready();

	if ((window as unknown as { __cafStub?: boolean }).__cafStub)
		(window as unknown as Record<string, unknown>).__receiver = { S, render, typeset, plainSubtitles, silent };
}

void start();
