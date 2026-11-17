// The receiver's own unpacking: a file (Matroska, WebM, MP4, MPEG-TS) or a VOD playlist (HLS,
// DASH) is read here and fed to the video element through MediaSource, so the relay only passes
// bytes through. Video goes to the TV's own decoders as it is. Audio goes to MediaSource when the
// TV takes it, else to the WebAssembly decoders (pcm.ts). Subtitles inside the file go to subs.ts.
//
// Nothing is checked before playing: MediaSource is asked for each audio track in each form it
// has (HE-AAC also as AAC-LC). A form it refuses is skipped for the rest of the session; a track
// it refuses in every form, or fails on while playing, plays through WebAssembly.

import type { Report } from '../errors';
import { codecString } from './codecs';
import { openDash } from './dash';
import { openHls } from './hls';
import { Matroska } from './mkv';
import { Mp4File } from './mp4';
import { streamInfo } from './flac-encode';
import { Mp4Muxer, createMuxer, mimeChoices, type Muxer } from './mux';
import { FLAC_MIME, FlacAudio, PRIMED, PcmAudio, decoderFor, flacSupported, type FlacFrame } from './pcm';
import { FetchError, LeaveToCaf, RangeSource, bytesOf } from './source';
import { Subtitles, type TypesetSink } from './subs';
import { TsFile } from './ts';
import type { Demuxer, Sample, Track } from './types';

/** The track number of the FLAC that decoded sound goes back to MediaSource as. */
const FLAC_ID = 1;

/** The FLAC track decoded sound goes back to MediaSource as: 16-bit stereo at `rate`. */
function flacTrack(rate: number): Track {
	return { id: FLAC_ID, kind: 'audio', codec: 'flac', config: streamInfo(rate, 2), sampleRate: rate, channels: 2, bitDepth: 16 };
}

/** Seconds read ahead of the playhead. */
const AHEAD = 30;
/**
 * Seconds read ahead while WebAssembly sound starts (until PACED ms after playing begins). On the
 * Chromecast TV, reading the whole 30 s at once while sound and picture start starves the audio
 * output: the context's clock fell 300 to 400 ms behind in the first second, and the sound with it.
 */
const START_AHEAD = 5;
const PACED = 2500;
/**
 * Seconds of FLAC sound buffered past the first picture before the picture goes in, after a start
 * or a seek, and the most ms the picture waits for it. The element plays as soon as both buffers
 * cover the playhead: with a few ms of sound it plays them, runs dry and waits, a short burst then a
 * gap before the sound continues.
 */
const FLAC_LEAD = 0.5;
const FLAC_WAIT = 2000;
/** Seconds kept behind the playhead. */
const BEHIND = 30;
/** Frames held to work out Matroska video decode times. */
const REORDER = 8;

/**
 * Audio MIME types addSourceBuffer refused in this session, and those of codecs outside the
 * common four that failed while playing: they are not asked for again.
 */
const refusedAudio = new Set<string>();

/**
 * The SourceBuffer for an audio track: each of its MIME types in turn, less those refused before.
 * A refusal is remembered in `refused`. No buffer: `why` says what was refused and the config.
 */
export function audioBuffer(
	t: Track,
	first: Sample | undefined,
	add: (mime: string) => SourceBuffer,
	refused: Set<string> = refusedAudio,
): { sb: SourceBuffer | null; muxer: Muxer | null; why: string | null } {
	const choices = mimeChoices(t, first);
	if (!choices.length) return { sb: null, muxer: null, why: `muxer: none for ${trackSetup(t)}` };
	const said: string[] = [];
	for (const mime of choices) {
		if (refused.has(mime)) {
			said.push(`${mime} refused earlier`);
			continue;
		}
		try {
			const sb = add(mime);
			return { sb, muxer: createMuxer(t, first, mime), why: null };
		} catch (e) {
			refused.add(mime);
			said.push(`${mime} refused: ${errText(e)}`);
		}
	}
	return { sb: null, muxer: null, why: `MediaSource ${said.join('; ')} (${trackSetup(t)})` };
}

export type Format = 'mkv' | 'mp4' | 'ts' | 'hls' | 'dash';

/** The format from the load's hint, the content type or the URL. Null: look at the bytes. */
export function formatOf(url: string, contentType = '', hint = ''): Format | null {
	const h = hint.toLowerCase();
	if (h === 'hls' || h === 'm3u8') return 'hls';
	if (h === 'dash' || h === 'mpd') return 'dash';
	if (h === 'mkv' || h === 'webm' || h === 'matroska') return 'mkv';
	if (h === 'mp4' || h === 'mov' || h === 'm4v') return 'mp4';
	if (h === 'ts' || h === 'm2ts' || h === 'mpegts') return 'ts';
	const ct = contentType.toLowerCase();
	if (/mpegurl/.test(ct)) return 'hls';
	if (/dash\+xml/.test(ct)) return 'dash';
	if (/matroska|webm/.test(ct)) return 'mkv';
	if (/mp2t/.test(ct)) return 'ts';
	let path = '';
	try {
		path = new URL(url).pathname.toLowerCase();
	} catch {
		path = url.toLowerCase();
	}
	if (/\.m3u8?$/.test(path)) return 'hls';
	if (/\.mpd$/.test(path)) return 'dash';
	if (/\.(mkv|mka|webm|mk3d)$/.test(path)) return 'mkv';
	if (/\.(mp4|m4v|m4a|mov)$/.test(path)) return 'mp4';
	if (/\.(ts|m2ts|mts)$/.test(path)) return 'ts';
	return null;
}

/** The format from a file's first bytes. */
export function sniff(b: Uint8Array): Format | null {
	if (b.length >= 4 && b[0] === 0x1a && b[1] === 0x45 && b[2] === 0xdf && b[3] === 0xa3) return 'mkv';
	const box = b.length >= 8 ? String.fromCharCode(b[4], b[5], b[6], b[7]) : '';
	if (/^(ftyp|moov|mdat|free|skip|wide|styp|sidx|moof)$/.test(box)) return 'mp4';
	if (b.length > 188 && b[0] === 0x47 && b[188] === 0x47) return 'ts';
	if (b.length > 196 && b[4] === 0x47 && b[196] === 0x47) return 'ts';
	const head = new TextDecoder().decode(b.subarray(0, 512)).trimStart();
	if (head.startsWith('#EXTM3U')) return 'hls';
	if (head.startsWith('<') && /<MPD[\s>]/.test(head)) return 'dash';
	return null;
}

/** Opens the demuxer for a URL. LeaveToCaf, FetchError or a parse error when it can't. */
export async function openDemuxer(url: string, contentType = '', hint = ''): Promise<{ demux: Demuxer; format: Format }> {
	let format = formatOf(url, contentType, hint);
	const height = typeof screen === 'object' ? Math.round(screen.height * (globalThis.devicePixelRatio || 1)) || 1080 : 1080;
	if (format === 'hls' || format === 'dash') {
		// The manifest's first bytes decide: a sender may name a DASH manifest HLS, or a file a
		// playlist.
		const head = sniff(await bytesOf(url, { range: [0, 1023], what: 'playlist' }));
		if (head === 'hls' || head === 'dash') format = head;
		else if (head) return openDemuxer(url, '', head);
		const demux = format === 'hls' ? await openHls(url, height) : await openDash(url, height);
		await demux.open();
		return { demux, format };
	}
	const src = new RangeSource(url);
	try {
		await src.open();
		// The first bytes win over the load's word for the container: a sender may name a
		// Matroska file MP4.
		const sniffed = sniff(await src.bytes(0, Math.min(1024, Number.isFinite(src.size) ? src.size : 1024)));
		if (sniffed === 'hls' || sniffed === 'dash') {
			src.close();
			return openDemuxer(url, '', sniffed);
		}
		format = sniffed ?? format;
		if (!format) throw new Error('not a format the receiver reads');
		const demux = format === 'mkv' ? new Matroska(src) : format === 'mp4' ? new Mp4File(src) : new TsFile(src);
		await demux.open();
		return { demux, format };
	} catch (e) {
		src.close();
		throw e;
	}
}

/**
 * Decode times for Matroska video, which stores presentation times only: each frame gets the
 * next-smallest presentation time of a short window, less a fixed delay, so frames reordered by
 * the encoder (B-frames) still decode before they show.
 */
export class DecodeTimes {
	private held: Sample[] = [];
	private times: number[] = [];
	private delay: number | null = null;
	private last = -Infinity;

	push(list: Sample[]): Sample[] {
		for (const s of list) {
			this.held.push(s);
			let i = this.times.length;
			while (i > 0 && this.times[i - 1] > s.pts) i--;
			this.times.splice(i, 0, s.pts);
		}
		const out: Sample[] = [];
		while (this.held.length > REORDER) out.push(this.next());
		return out;
	}

	flush(): Sample[] {
		const out: Sample[] = [];
		while (this.held.length) out.push(this.next());
		return out;
	}

	reset() {
		this.held = [];
		this.times = [];
		this.last = -Infinity;
	}

	private next(): Sample {
		if (this.delay == null) {
			let d = 0;
			this.held.forEach((s, i) => (d = Math.max(d, this.times[i] - s.pts)));
			this.delay = d;
		}
		const s = this.held.shift()!;
		const t = this.times.shift()!;
		let dts = Math.min(t - this.delay, s.pts);
		dts = Math.max(dts, this.last + 1e-4, 0);
		this.last = dts;
		return { ...s, dts };
	}
}

/** One SourceBuffer and the muxer that writes for it. Appends wait for each other. */
class Output {
	constructor(
		readonly sb: SourceBuffer,
		public muxer: Muxer,
		readonly track: Track,
	) {}

	private done(): Promise<void> {
		return new Promise((resolve, reject) => {
			const end = () => {
				this.sb.removeEventListener('updateend', end);
				this.sb.removeEventListener('error', fail);
				resolve();
			};
			const fail = () => {
				this.sb.removeEventListener('updateend', end);
				this.sb.removeEventListener('error', fail);
				reject(new Error('append failed'));
			};
			this.sb.addEventListener('updateend', end);
			this.sb.addEventListener('error', fail);
		});
	}

	async idle() {
		while (this.sb.updating) await new Promise((r) => this.sb.addEventListener('updateend', r, { once: true }));
	}

	async append(data: Uint8Array) {
		await this.idle();
		this.sb.appendBuffer(data as Uint8Array<ArrayBuffer>);
		await this.done();
	}

	async remove(from: number, to: number) {
		if (to <= from) return;
		await this.idle();
		this.sb.remove(from, to);
		await this.done();
	}

	/** The buffered range holding `t`, if any. */
	rangeAt(t: number): [number, number] | null {
		try {
			const b = this.sb.buffered;
			for (let i = 0; i < b.length; i++) if (t >= b.start(i) - 0.05 && t < b.end(i)) return [b.start(i), b.end(i)];
		} catch {
			// removed from its MediaSource
		}
		return null;
	}

	ranges(): [number, number][] {
		const out: [number, number][] = [];
		try {
			const b = this.sb.buffered;
			for (let i = 0; i < b.length; i++) out.push([b.start(i), b.end(i)]);
		} catch {
			// removed
		}
		return out;
	}
}

export interface UnpackerOptions {
	url: string;
	contentType?: string;
	/** The load's container word (`mkv`, `mp4`, `hls`). */
	container?: string;
	/** Where playback starts, seconds. */
	start?: number;
	/** The audio track to play, counted from 0 in the file's audio tracks. */
	audio?: number | null;
	/** The sender's word for the video codec (`h264`, `video:h.264 high 10`), sent back in errors. */
	videoCodec?: string | null;
	/** The subtitle track to show, counted from 0 in the file's subtitle tracks. */
	text?: number | null;
	/** The receiver's base URL, where pcm/ is. */
	base: string;
	/** The typeset renderer for ASS inside the file, when it can run. */
	sink: () => TypesetSink | null;
	/** An error the sender should hear about. Subtitle errors keep playing; the others end it. */
	report(r: Report): void;
}

/** A failure already worded for the sender. */
export class Refused extends Error {
	constructor(readonly report: Report) {
		super(report.detail);
	}
}

function report(part: Report['part'], detail: string, extra: Partial<Report> = {}): Report {
	return { part, codec: null, detail, fetched: null, status: null, detailed: null, shaka: null, lost: false, ...extra };
}

/** The engine's names for video codecs; anything else is `video:<name>`. */
const ENGINE_VIDEO = /^(h264|hevc|vp9|av1)$/;

/**
 * The video track the TV can't take, worded for the sender, or null. Read from the container's
 * codec id before anything plays, so a picture the TV can't decode never plays as sound alone.
 */
export function videoRefusal(t: Track, senderWord?: string | null): Report | null {
	const muxer = createMuxer(t);
	if (!muxer) {
		const codec = senderWord || (ENGINE_VIDEO.test(t.codec) ? t.codec : `video:${t.codec}`);
		return report('video', `the TV has no decoder for ${t.codec} video`, { codec });
	}
	if (typeof MediaSource !== 'undefined' && !MediaSource.isTypeSupported(muxer.mime))
		return report('video', `MediaSource doesn't take ${muxer.mime}`, { codec: senderWord || (codecString(t) ?? t.codec) });
	return null;
}

/** The base of a track's codec string as the TV names it (`hvc1`, `ec-3`), or the codec word. */
function codecName(t: Track): string {
	return codecString(t)?.split('.')[0] ?? t.codec;
}

/** Stats for measuring on the TV. */
export interface UnpackerStats {
	format: Format;
	video: string | null;
	audio: string | null;
	/** wasm: decoded here and played by the worklet; flac: decoded here and played by MediaSource as FLAC. */
	audioPath: 'mse' | 'wasm' | 'flac' | null;
	text: string | null;
	readTo: number;
	buffered: [number, number][];
	seeks: number;
	/** Spans stepped over because their segments never came. */
	skipped: [number, number][];
	pcm: PcmAudio['stats'] | null;
	/** The audio track as the receiver read it, for the dev log. */
	audioSetup: string | null;
	/** Why the audio plays through WebAssembly, when it does. */
	audioWhy: string | null;
	/** How long the last start held the picture for WebAssembly sound, and what let it go. */
	audioGate: string | null;
	/** Milliseconds from the start of the load to its steps, once the first frame is up. */
	timing: string | null;
	/** The WebAssembly sound's clock readings in the first seconds after a start. */
	clockTrace: string[] | null;
}

/** A track's setup in one line: codec string, MIME, rate, channels and config bytes. */
export function trackSetup(t: Track, mime?: string | null): string {
	const hex = t.config ? Array.from(t.config.subarray(0, 48), (b) => b.toString(16).padStart(2, '0')).join('') : 'none';
	const more = t.config && t.config.length > 48 ? `+${t.config.length - 48}` : '';
	return `${t.codec} ${codecString(t) ?? '?'} mime ${mime ?? '?'} rate ${t.sampleRate ?? '?'} ch ${t.channels ?? '?'}${t.unframed ? ' unframed' : ''} config ${hex}${more}`;
}

/** One load, unpacked into a MediaSource whose object URL CAF plays. */
export class Unpacker {
	url: string;
	readonly format: Format;
	private ms: MediaSource;
	private vout: Output | null = null;
	private aout: Output | null = null;
	private pcm: PcmAudio | null = null;
	/** WebAssembly sound started before the first samples, for a track MediaSource won't take. */
	private early: { id: number; made: Promise<PcmAudio | FlacAudio> } | null = null;
	/** Sound decoded in WebAssembly that goes back to MediaSource as FLAC, through `aout`. */
	private flac: FlacAudio | null = null;
	/** The sample rate of the FLAC init segment appended, null before the first one. */
	private flacRate: number | null = null;
	/** FLAC appends, one after another. */
	private flacChain: Promise<void> = Promise.resolve();
	/** FLAC frames from the decoder not appended yet: appended together, one segment per rate. */
	private flacQueue: { rate: number; frames: FlacFrame[]; g: number }[] = [];
	/** After a start or a seek, the picture held until FLAC sound leads it (FLAC_LEAD). */
	private flacHold: { video: Sample[]; since: number } | null = null;
	private fixer: DecodeTimes | null = null;
	private readonly subs: Subtitles;
	private videoTrack: Track | null;
	private audioTrack: Track | null;
	private textTrack: Track | null = null;
	/** Samples read before the outputs exist. */
	private held: Sample[] | null = [];
	private gen = 0;
	private seekTo: number | null = null;
	/** Where the engine last stepped the video over a gap itself (tick). */
	private stepTo: number | null = null;
	/** The read loop runs; one per Unpacker. */
	private started = false;
	/** The last seek's target and where reading started for it. */
	private target = 0;
	private runFrom = 0;
	private readTo = 0;
	private atEnd = false;
	/** After a seek, video starts at a keyframe (MPEG-TS seeks land anywhere). */
	private needKey = true;
	private closed = false;
	private failed = false;
	private seeks = 0;
	private wake: (() => void) | null = null;
	private timer = 0;
	private swapping = false;
	private audioWhy: string | null = null;
	private audioGate: string | null = null;
	/** performance.now() at the start of the load, and when its steps happened. */
	private t0 = performance.now();
	private setupAt: number | null = null;
	private gateAt: number | null = null;
	private frameAt: number | null = null;
	private playingAt: number | null = null;
	/** When playing began after the last start or seek, for the paced read-ahead (see lead()). */
	private pacedFrom: number | null = null;
	/** The WebAssembly sound was started before the first samples (see startEarly). */
	private pcmEarly = false;
	private flacEarly = false;
	/** An audio switch is changing the SourceBuffer: reading waits. */
	private switching = false;
	private listeners: [EventTarget, string, EventListener, boolean][] = [];
	/** The first seconds of a FLAC start, step by step, for the dev log (see note()). */
	private startTrace: string[] | null = null;
	private traceTimer = 0;

	private constructor(
		private readonly demux: Demuxer,
		format: Format,
		private readonly video: HTMLVideoElement,
		private readonly o: UnpackerOptions,
	) {
		this.format = format;
		const tracks = demux.tracks;
		this.videoTrack = tracks.find((t) => t.kind === 'video') ?? null;
		const audio = tracks.filter((t) => t.kind === 'audio');
		this.audioTrack = (o.audio != null ? audio[o.audio] : null) ?? audio.find((t) => t.default) ?? audio[0] ?? null;
		const text = tracks.filter((t) => t.kind === 'text');
		this.textTrack = o.text != null ? (text[o.text] ?? null) : null;
		if (this.videoTrack && demux instanceof Matroska && /^(h264|hevc|mpeg2|mpeg4)$/.test(this.videoTrack.codec))
			this.fixer = new DecodeTimes();
		this.subs = new Subtitles(video, o.sink());
		this.ms = new MediaSource();
		this.url = URL.createObjectURL(this.ms);
		this.target = this.runFrom = this.readTo = Math.max(0, o.start ?? 0);
	}

	/**
	 * Opens the stream. Throws Refused for video the TV can't decode (the load ends there), or
	 * LeaveToCaf, FetchError or a parse error (the caller hands the load to CAF).
	 */
	static async open(video: HTMLVideoElement, o: UnpackerOptions): Promise<Unpacker> {
		const t0 = performance.now();
		const { demux, format } = await openDemuxer(o.url, o.contentType, o.container);
		if (!demux.tracks.some((t) => t.kind === 'video' || t.kind === 'audio')) {
			demux.close();
			throw new Error('no video or audio tracks');
		}
		const v = demux.tracks.find((t) => t.kind === 'video');
		const refused = v ? videoRefusal(v, o.videoCodec) : null;
		if (refused) {
			demux.close();
			throw new Refused(refused);
		}
		const u = new Unpacker(demux, format, video, o);
		u.t0 = t0;
		await u.selectTracks();
		u.startEarly();
		u.begin();
		return u;
	}

	get tracks() {
		return this.demux.tracks;
	}

	get duration() {
		return this.demux.duration;
	}

	stats(): UnpackerStats {
		return {
			format: this.format,
			video: this.videoTrack ? codecName(this.videoTrack) : null,
			audio: this.audioTrack?.codec ?? null,
			audioPath: this.flac ? 'flac' : this.pcm ? 'wasm' : this.aout ? 'mse' : null,
			text: this.textTrack?.codec ?? null,
			readTo: this.readTo,
			buffered: this.vout?.ranges() ?? this.aout?.ranges() ?? [],
			seeks: this.seeks,
			skipped: this.demux.skipped ?? [],
			pcm: this.pcm ? this.pcm.stats : null,
			audioSetup: this.audioTrack ? `${this.format}/${this.demux.container ?? this.format} ${trackSetup(this.audioTrack, this.aout?.muxer.mime)}` : null,
			audioWhy: this.pcm ? this.audioWhy : null,
			audioGate: this.pcm || this.flac ? this.audioGate : null,
			timing: this.timing(),
			clockTrace: this.pcm ? this.pcm.trace : this.startTrace,
		};
	}

	private timing(): string | null {
		if (this.frameAt == null) return null;
		const ms = (t: number | null | undefined) => (t == null ? '-' : `${Math.round(t - this.t0)}`);
		let line = `load to setup ${ms(this.setupAt)} ms, first frame ${ms(this.frameAt)} ms, playing ${ms(this.playingAt)} ms`;
		if (this.flac) line += `, first FLAC frames ${ms(this.flac.firstFrames)} ms, picture let go ${ms(this.gateAt)} ms (${this.audioGate ?? '-'}); audio setup ${this.flacEarly ? 'early' : 'after video'}: ${this.flac.setup}`;
		if (this.pcm) {
			const t = this.pcm.timing;
			const l = this.pcm.latency;
			const sound = t.sound != null && this.playingAt != null ? `${Math.round(t.sound - this.playingAt)}` : '-';
			line += `, sound decoded ${ms(t.decoded)} ms, ${Math.round(PRIMED * 1000)} ms queued ${ms(t.primed)} ms, picture let go ${ms(this.gateAt)} ms; playing to first sound ${sound} ms; latency output ${Math.round(l.output * 1000)} base ${Math.round(l.base * 1000)} ms; audio setup ${this.pcmEarly ? 'early' : 'after video'}: ${this.pcm.setup}`;
		}
		return line;
	}

	/** The WebAssembly audio's level tap, for the testing build. Null when MSE plays the sound. */
	pcmAnalyser() {
		return this.pcm?.analyser() ?? null;
	}

	/** Decoder CPU time, for measuring. */
	pcmCpu() {
		return this.pcm?.cpu() ?? null;
	}

	/** Decoder and FLAC encoder CPU time, for measuring. Null unless sound goes to MediaSource as FLAC. */
	flacCpu() {
		return this.flac?.cpu() ?? null;
	}

	private async selectTracks() {
		const ids = [this.videoTrack, this.audioTrack, this.textTrack].filter((t): t is Track => !!t).map((t) => t.id);
		await this.demux.select?.(ids);
		// Segmented streams learn an audio rendition's setup only once it is selected.
		const by = (t: Track | null) => (t ? (this.demux.tracks.find((x) => x.id === t.id) ?? t) : null);
		this.audioTrack = by(this.audioTrack);
		this.videoTrack = by(this.videoTrack);
	}

	private on(target: EventTarget, type: string, fn: EventListener, capture = false) {
		target.addEventListener(type, fn, capture);
		this.listeners.push([target, type, fn, capture]);
	}

	private begin() {
		// The first frame on screen, for the load timing.
		const v = this.video as HTMLVideoElement & { requestVideoFrameCallback?: (cb: (now: number) => void) => number };
		if (v.requestVideoFrameCallback) v.requestVideoFrameCallback((now) => (this.frameAt ??= now));
		this.on(this.video, 'playing', () => {
			this.playingAt ??= performance.now();
			this.pacedFrom ??= performance.now();
			if (!v.requestVideoFrameCallback) this.frameAt ??= this.playingAt;
		});
		for (const type of ['loadedmetadata', 'canplay', 'playing', 'waiting', 'seeking', 'seeked', 'pause', 'stalled'])
			this.on(this.video, type, () => this.note(type));
		this.on(this.ms, 'sourceopen', () => this.opened(), false);
		// CAF unloaded the element (a stop, or another load).
		this.on(this.ms, 'sourceclose', () => !this.swapping && this.destroy(), false);
		this.on(this.video, 'seeking', () => this.onSeeking());
		// Before CAF hears a media error: an audio failure goes to WebAssembly instead.
		this.on(window, 'error', (e) => this.onMediaError(e), true);
		for (const type of ['emptied', 'abort', 'loadstart', 'pause', 'waiting', 'stalled', 'suspend'])
			this.on(window, type, (e) => {
				if (this.swapping && e.target === this.video) e.stopPropagation();
			}, true);
		this.timer = window.setInterval(() => this.tick(), 250);
		if (this.textTrack) void this.subs.show(this.textTrack, () => this.attachments());
	}

	private attachments() {
		return this.demux.attachments?.() ?? Promise.resolve([]);
	}

	private opened() {
		// A MediaSource that ended opens again when a seek appends to it, and says sourceopen each
		// time. A second read loop on the same file would feed old audio after the seek.
		if (this.closed || this.started || this.ms.readyState !== 'open') return;
		this.started = true;
		this.setDuration();
		if (this.target > 0) this.seekTo = this.target;
		void this.run();
	}

	private setDuration() {
		const d = this.demux.duration;
		if (!Number.isFinite(d) || d <= 0) return;
		try {
			if (this.ms.readyState === 'open' && !this.ms.sourceBuffers.length) this.ms.duration = d;
			else if (this.ms.readyState === 'open' && ![...this.ms.sourceBuffers].some((b) => b.updating)) this.ms.duration = Math.max(d, this.bufferedEnd());
		} catch {
			// set once the buffers are idle
		}
	}

	private bufferedEnd() {
		let end = 0;
		for (const o of [this.vout, this.aout]) for (const [, e] of o?.ranges() ?? []) end = Math.max(end, e);
		return end;
	}

	// ---------- Reading ----------

	private nap(ms = 250): Promise<void> {
		return new Promise((resolve) => {
			const t = setTimeout(done, ms);
			function done() {
				clearTimeout(t);
				resolve();
			}
			this.wake = done;
		});
	}

	private poke() {
		const w = this.wake;
		this.wake = null;
		w?.();
	}

	private async run() {
		while (!this.closed && !this.failed) {
			if (this.seekTo != null) {
				const t = this.seekTo;
				this.seekTo = null;
				try {
					await this.seekNow(t);
				} catch (e) {
					if (!this.closed) this.fail(e);
					return;
				}
				continue;
			}
			if (this.switching) {
				await this.nap();
				continue;
			}
			const g = this.gen;
			const playhead = this.video.currentTime;
			if (this.atEnd || (this.held == null && this.readTo - Math.max(playhead, this.target) > this.lead())) {
				await this.nap();
				continue;
			}
			let batch: Sample[] | null;
			try {
				batch = await this.demux.read();
			} catch (e) {
				if (g !== this.gen || this.closed) continue;
				this.fail(e);
				return;
			}
			if (g !== this.gen || this.closed) continue;
			try {
				if (!batch) await this.finish(g);
				else await this.route(batch, g);
			} catch (e) {
				if (g !== this.gen || this.closed) continue;
				this.fail(e);
				return;
			}
		}
	}

	/** Seconds to read ahead now: less while WebAssembly sound starts. */
	private lead(): number {
		if (!this.pcm && !this.early) return AHEAD;
		return this.pacedFrom != null && performance.now() - this.pacedFrom > PACED ? AHEAD : START_AHEAD;
	}

	private async seekNow(t: number) {
		const g = ++this.gen;
		// FLAC frames of the old position still on their way are dropped from here on.
		this.flac?.seek(t);
		this.flacQueue = [];
		this.flacHold = this.flac ? { video: [], since: performance.now() } : null;
		this.pacedFrom = null;
		this.seeks++;
		this.target = t;
		const start = await this.demux.seek(t);
		if (g !== this.gen) return;
		this.runFrom = Math.min(start, t);
		this.readTo = start;
		this.atEnd = false;
		this.needKey = true;
		this.fixer?.reset();
		this.vout?.muxer.reset();
		this.aout?.muxer.reset();
		for (const o of [this.vout, this.aout]) {
			if (!o) continue;
			await o.idle();
			try {
				if (this.ms.readyState === 'open') o.sb.abort();
			} catch {
				// nothing to abort
			}
		}
		this.pcm?.seek(t);
		this.subs.seek();
	}

	private async route(batch: Sample[], g: number) {
		const v = this.videoTrack?.id;
		const a = this.audioTrack?.id;
		const x = this.textTrack?.id;
		if (x != null) {
			const text = batch.filter((s) => s.track === x);
			if (text.length) this.subs.push(text);
		}
		const media = batch.filter((s) => s.track === v || s.track === a);
		if (!media.length) return;
		for (const s of media) if (s.track === (v ?? a)) this.readTo = Math.max(this.readTo, s.pts);
		if (this.held) {
			this.held.push(...media);
			const haveVideo = v == null || this.held.some((s) => s.track === v);
			const haveAudio = a == null || this.held.some((s) => s.track === a);
			if ((haveVideo && haveAudio) || this.held.length > 2000) await this.outputs(g);
			return;
		}
		await this.feed(media, g);
	}

	/** Makes the SourceBuffers (or the WebAssembly audio) once the first samples show their setup. */
	private async outputs(g: number) {
		const held = this.held ?? [];
		const vt = this.videoTrack;
		const at = this.audioTrack;
		if (vt) {
			const first = held.find((s) => s.track === vt.id);
			const muxer = createMuxer(vt, first);
			if (!muxer) throw new Refused(report('video', `no MediaSource form for ${vt.codec}`, { codec: vt.codec }));
			try {
				this.vout = new Output(this.ms.addSourceBuffer(muxer.mime), muxer, vt);
			} catch (e) {
				throw new Refused(report('video', `MediaSource refused ${muxer.mime}: ${errText(e)}`, { codec: codecName(vt) }));
			}
		}
		if (at) await this.makeAudio(at, held.find((s) => s.track === at.id));
		this.setupAt ??= performance.now();
		this.held = null;
		this.setDuration();
		if (this.vout) await this.vout.append(this.vout.muxer.init());
		if (this.aout && !this.flac) await this.aout.append(this.aout.muxer.init());
		if (g !== this.gen) return;
		await this.feed(held, g);
	}

	/**
	 * Starts the WebAssembly sound while the first samples are read, when no MediaSource form of the
	 * track is supported: its setup then runs alongside the video's instead of after it. AAC waits for
	 * its first frame, which can change its setup.
	 */
	private startEarly() {
		const t = this.audioTrack;
		if (!t || t.codec === 'aac' || !decoderFor(t)) return;
		const supported = (mime: string) => !refusedAudio.has(mime) && (globalThis.MediaSource?.isTypeSupported?.(mime) ?? false);
		if (mimeChoices(t).some(supported)) return;
		const made: Promise<PcmAudio | FlacAudio> = this.canFlac()
			? FlacAudio.create(this.o.base, t, this.target, (rate, frames) => this.onFlac(rate, frames))
			: PcmAudio.create(this.o.base, t, this.video, this.target);
		made.catch(() => {});
		this.early = { id: t.id, made };
	}

	/** The early WebAssembly sound for `t`, when there is one. */
	private takeEarly(t: Track): Promise<PcmAudio | FlacAudio> | null {
		const e = this.early;
		this.early = null;
		if (!e) return null;
		if (e.id === t.id) return e.made;
		void e.made.then((p) => p.destroy(), () => {});
		return null;
	}

	/** Whether decoded sound can go back to MediaSource as FLAC. */
	private canFlac() {
		return !refusedAudio.has(FLAC_MIME) && flacSupported();
	}

	/**
	 * Decodes `t` in WebAssembly into an audio SourceBuffer as FLAC. False when MediaSource refuses
	 * the FLAC SourceBuffer (the worklet plays the sound then).
	 */
	private async startFlac(t: Track): Promise<boolean> {
		let sb: SourceBuffer;
		try {
			sb = this.ms.addSourceBuffer(FLAC_MIME);
		} catch (e) {
			refusedAudio.add(FLAC_MIME);
			this.audioWhy = `${this.audioWhy ?? ''}; ${FLAC_MIME} refused: ${errText(e)}`;
			return false;
		}
		const rate = t.sampleRate || 48000;
		this.aout = new Output(sb, new Mp4Muxer(flacTrack(rate), FLAC_MIME), flacTrack(rate));
		this.flacRate = null;
		const early = this.takeEarly(t);
		let made: FlacAudio | null = null;
		if (early) {
			const e = await early.catch(() => null);
			if (e instanceof FlacAudio) {
				made = e;
				this.flacEarly = true;
			} else e?.destroy();
		}
		try {
			made ??= await FlacAudio.create(this.o.base, t, this.target, (r, frames) => this.onFlac(r, frames));
		} catch (e) {
			throw new Refused(report('audio', `WebAssembly ${t.codec}: ${errText(e)}`, { codec: t.codec }));
		}
		if (this.closed) {
			made.destroy();
			return true;
		}
		this.flac = made;
		this.flac.seek(this.target);
		this.flacHold = { video: [], since: performance.now() };
		return true;
	}

	/** FLAC frames from the decoder: appended in order, after the init segment for their rate. */
	private onFlac(rate: number, frames: FlacFrame[]) {
		const g = this.gen;
		this.flacQueue.push({ rate, frames, g });
		this.flacChain = this.flacChain
			.then(() => this.putQueued())
			.catch((e) => {
				if (g === this.gen && !this.closed) this.fail(e);
			});
	}

	/** Appends every queued FLAC frame of this start, in as few appends as the rates allow. */
	private async putQueued() {
		while (this.flacQueue.length) {
			const g = this.flacQueue[0].g;
			const rate = this.flacQueue[0].rate;
			let n = 1;
			while (n < this.flacQueue.length && this.flacQueue[n].g === g && this.flacQueue[n].rate === rate) n++;
			const frames = this.flacQueue.splice(0, n).flatMap((q) => q.frames);
			await this.putFlac(rate, frames, g);
		}
	}

	private async putFlac(rate: number, frames: FlacFrame[], g: number) {
		const o = this.aout;
		if (!o || g !== this.gen || this.closed) return;
		if (this.flacRate !== rate) {
			const t = flacTrack(rate);
			o.muxer = new Mp4Muxer(t, FLAC_MIME);
			await o.append(o.muxer.init());
			this.flacRate = rate;
		}
		const samples: Sample[] = frames.map((f) => ({ track: FLAC_ID, pts: f.pts, dts: f.pts, duration: f.samples / rate, key: true, data: f.data }));
		await this.put(o, o.muxer.segment(samples), g);
	}

	private async makeAudio(t: Track, first: Sample | undefined) {
		const got = audioBuffer(t, first, (mime) => this.ms.addSourceBuffer(mime));
		if (got.sb && got.muxer) {
			this.aout = new Output(got.sb, got.muxer, t);
			const early = this.takeEarly(t);
			if (early) void early.then((p) => p.destroy(), () => {});
			return;
		}
		this.audioWhy = got.why;
		console.info('audio to WebAssembly:', this.audioWhy);
		if (this.canFlac() && (await this.startFlac(t))) return;
		await this.startPcm(t);
	}

	private async startPcm(t: Track) {
		if (!decoderFor(t)) throw new Refused(report('audio', `no decoder for ${t.codec}`, { codec: t.codec }));
		const early = this.takeEarly(t);
		if (early) {
			try {
				const pcm = await early;
				if (this.closed) return pcm.destroy();
				if (!(pcm instanceof PcmAudio)) throw pcm.destroy();
				this.pcm = pcm;
				this.pcm.seek(this.target);
				this.pcmEarly = true;
				return;
			} catch {
				// Made again below, where a failure counts.
			}
		}
		try {
			this.pcm = await PcmAudio.create(this.o.base, t, this.video, this.target);
		} catch (e) {
			throw new Refused(report('audio', `WebAssembly ${t.codec}: ${errText(e)}`, { codec: t.codec }));
		}
	}

	private async feed(samples: Sample[], g: number) {
		const v = this.videoTrack?.id;
		const a = this.audioTrack?.id;
		let video = samples.filter((s) => s.track === v);
		const audio = samples.filter((s) => s.track === a);
		if (this.needKey && video.length) {
			const k = video.findIndex((s) => s.key);
			video = k < 0 ? [] : video.slice(k);
			if (k >= 0) this.needKey = false;
		}
		if (this.fixer && video.length) video = this.fixer.push(video);
		let pushed = false;
		// WebAssembly sound: the first picture after a start or a seek waits for decoded sound, so
		// both begin together instead of the sound starting late and skipping to catch up.
		if (this.pcm && audio.length && video.length && !this.pcm.isPrimed) {
			this.pcm.push(audio);
			pushed = true;
			const since = performance.now();
			const how = await this.pcm.ready();
			this.audioGate = `${Math.round(performance.now() - since)} ms ${how}`;
			this.gateAt ??= performance.now();
			if (g !== this.gen) return;
		}
		// FLAC sound: the first picture after a start or a seek waits until the sound leads it.
		const hold = this.flacHold;
		if (this.flac && hold && this.vout) {
			if (audio.length) this.flac.push(audio);
			pushed = true;
			hold.video.push(...video);
			video = [];
			const first = hold.video[0]?.pts;
			const late = performance.now() - hold.since > FLAC_WAIT;
			if (first != null && (late || this.flac.bufferedTo >= first + FLAC_LEAD)) {
				const since = performance.now();
				const how = await this.flacReaches(first + FLAC_LEAD, Math.max(0, hold.since + FLAC_WAIT - since));
				if (g !== this.gen || hold !== this.flacHold) return;
				this.audioGate = `${Math.round(performance.now() - hold.since)} ms ${how}`;
				this.gateAt ??= performance.now();
				this.note(`gate ${how}`);
				this.flacHold = null;
				video = hold.video;
			}
		}
		if (video.length && this.vout) await this.put(this.vout, this.vout.muxer.segment(video), g);
		if (audio.length && !pushed) {
			if (this.flac) this.flac.push(audio);
			else if (this.pcm) this.pcm.push(audio);
			else if (this.aout) await this.put(this.aout, this.aout.muxer.segment(audio), g);
		}
		await this.evict();
	}

	/** Waits until FLAC sound is buffered from the playhead's range to `t`, or `ms` passed. */
	private async flacReaches(t: number, ms: number): Promise<'sound' | 'timeout'> {
		const until = performance.now() + ms;
		for (;;) {
			if (this.aout?.rangeAt(t - 0.03)) return 'sound';
			if (performance.now() >= until || this.closed) return 'timeout';
			await new Promise((r) => setTimeout(r, 20));
		}
	}

	private async put(o: Output, data: Uint8Array | null, g: number) {
		if (!data || g !== this.gen) return;
		for (let attempt = 0; ; attempt++) {
			try {
				await o.append(data);
				this.note(o === this.aout ? `A${data.length}` : `V${data.length}`);
				return;
			} catch (e) {
				if ((e as DOMException)?.name !== 'QuotaExceededError' || attempt > 20 || g !== this.gen) throw e;
				// Full: drop what is behind, or wait for the playhead to move on.
				await this.evict(5);
				await this.nap(500);
			}
		}
	}

	/** Drops buffered media far behind or far ahead of the playhead. */
	private async evict(behind = BEHIND) {
		const t = this.video.currentTime;
		for (const o of [this.vout, this.aout]) {
			if (!o) continue;
			for (const [s, e] of o.ranges()) {
				if (s < t - behind) await o.remove(s, Math.min(e, t - behind));
				if (e > t + AHEAD * 2 && e > this.readTo + 1) await o.remove(Math.max(s, t + AHEAD * 2), e);
			}
		}
		this.runFrom = Math.max(this.runFrom, t - behind);
	}

	private async finish(g: number) {
		if (this.held) await this.outputs(g);
		if (g !== this.gen) return;
		// A stream shorter than the FLAC lead: its held picture goes in with the sound there is.
		const hold = this.flacHold;
		if (hold?.video.length && this.vout) {
			this.flacHold = null;
			await this.put(this.vout, this.vout.muxer.segment(hold.video), g);
		}
		if (this.fixer && this.vout) await this.put(this.vout, this.vout.muxer.segment(this.fixer.flush()), g);
		if (this.vout) await this.put(this.vout, this.vout.muxer.flush(), g);
		if (this.flac) {
			await this.flac.drain();
			await this.flacChain;
			if (g !== this.gen) return;
		}
		if (this.aout) await this.put(this.aout, this.aout.muxer.flush(), g);
		if (g !== this.gen) return;
		this.atEnd = true;
		await this.vout?.idle();
		await this.aout?.idle();
		try {
			if (this.ms.readyState === 'open') this.ms.endOfStream();
		} catch {
			// a buffer was busy; the next tick ends it
		}
	}

	// ---------- The element ----------

	/** Whether the picture, and the sound when MediaSource plays it, are buffered at `t`. */
	private buffered(t: number) {
		const main = this.vout ?? this.aout;
		return !!main && !!main.rangeAt(t) && (!this.aout || !!this.aout.rangeAt(t));
	}

	private covered(t: number) {
		if (this.pcm) return false;
		// Already read, or being read now.
		if (t >= this.runFrom - 0.01 && t <= this.readTo + 2) return true;
		return this.buffered(t);
	}

	private onSeeking() {
		if (this.closed || this.swapping) return;
		const t = this.video.currentTime;
		// The start position CAF sets once the metadata is in.
		if (Math.abs(t - this.target) < 0.05 && this.seeks <= 1 && this.video.played.length === 0) return;
		// The engine's own step over a gap lands on buffered media: no new read. For WebAssembly
		// sound a new read would also flush the sound already decoded, and start with a gap.
		const step = this.stepTo;
		this.stepTo = null;
		if (step != null && Math.abs(t - step) < 0.02) return;
		if (this.covered(t)) return;
		this.seekTo = t;
		this.gen++;
		this.demux.interrupt?.();
		this.poke();
	}

	/**
	 * Notes a step of a FLAC start for the dev log: ms since the load, what happened, the playhead,
	 * readyState, the element's audio bytes decoded, and the video and audio buffered ranges. Runs
	 * until 3 s after playing begins; a 50 ms sampler adds lines when anything changed.
	 */
	private note(what: string) {
		if (!this.flac && !this.early && !this.startTrace) return;
		if (this.startTrace == null) {
			this.startTrace = [];
			this.traceTimer = window.setInterval(() => this.note('.'), 50);
		}
		const tr = this.startTrace;
		if (this.playingAt != null && performance.now() - this.playingAt > 3000 || tr.length > 400) {
			window.clearInterval(this.traceTimer);
			return;
		}
		const v = this.video as HTMLVideoElement & { webkitAudioDecodedByteCount?: number };
		const r = (o: Output | null) => (o ? o.ranges().map(([a, b]) => `${a.toFixed(3)}-${b.toFixed(3)}`).join(',') || '-' : 'none');
		const state = `t${v.currentTime.toFixed(3)} r${v.readyState}${v.paused ? 'p' : ''} ab${v.webkitAudioDecodedByteCount ?? '?'} v${r(this.vout)} a${r(this.aout)}`;
		if (what === '.' && this.lastNote === state) return;
		this.lastNote = state;
		tr.push(`${Math.round(performance.now() - this.t0)}:${what} ${state}`);
	}

	private lastNote = '';

	private tick() {
		if (this.closed) return;
		const v = this.video;
		// A small gap at the start or after a seek: step over it.
		if (!v.seeking && v.readyState < 3 && this.vout) {
			const t = v.currentTime;
			if (!this.buffered(t)) {
				// Past a skipped segment the gap runs to the end of its span.
				const span = this.demux.skipped?.find(([a, b]) => t >= a - 1.5 && t < b);
				const reach = span ? span[1] - t + 1.5 : 1.5;
				const next = [this.vout, this.aout]
					.flatMap((o) => o?.ranges() ?? [])
					.map(([s]) => s + 0.01)
					.filter((s) => s > t && s - t < reach && this.buffered(s))
					.sort((a, b) => a - b)[0];
				if (next != null) {
					this.note(`step>${next.toFixed(3)}`);
					this.stepTo = next;
					v.currentTime = next;
				}
			}
		}
		if (this.atEnd && this.ms.readyState === 'open' && !this.vout?.sb.updating && !this.aout?.sb.updating) {
			try {
				this.ms.endOfStream();
			} catch {
				// next tick
			}
		}
		// A seek that never stopped playing says no 'playing' again.
		if (this.pacedFrom == null && !v.paused && !v.seeking && v.readyState >= 3) this.pacedFrom = performance.now();
		if (this.readTo - v.currentTime < AHEAD) this.poke();
	}

	/** A media error: an audio failure in MediaSource moves the audio to WebAssembly once. */
	private onMediaError(e: Event) {
		if (e.target !== this.video || this.closed) return;
		if (this.swapping) {
			e.stopPropagation();
			return;
		}
		const message = this.video.error?.message ?? '';
		const at = this.audioTrack;
		const audioSaid = /audio/i.test(message);
		const videoSaid = /video/i.test(message);
		const exotic = !!at && !/^(aac|opus|mp3|vorbis)$/.test(at.codec);
		if (this.aout && at && (audioSaid || (!videoSaid && exotic))) {
			e.stopPropagation();
			// The common four play through MediaSource on every TV, so one stream's failure moves
			// only that stream; another codec's failure says the TV can't decode it.
			if (exotic) refusedAudio.add(this.aout.muxer.mime);
			this.audioWhy = `media error on ${this.aout.muxer.mime}: ${message || 'no message'}`;
			console.info('audio to WebAssembly after', message);
			void this.toPcm().catch((err) => this.fail(err));
			return;
		}
		if (videoSaid && this.videoTrack && !this.failed) {
			// CAF hears the error too; the receiver sends this, the more exact one, first.
			this.failed = true;
			this.o.report(report('video', message, { codec: codecName(this.videoTrack) }));
		}
	}

	/** Starts the element again on a new MediaSource with the audio in WebAssembly. */
	private async toPcm() {
		const v = this.video;
		const at = v.currentTime;
		const vt = this.videoTrack;
		this.swapping = true;
		this.gen++;
		const old = this.url;
		this.ms = new MediaSource();
		this.url = URL.createObjectURL(this.ms);
		const open = new Promise((r) => this.ms.addEventListener('sourceopen', r, { once: true }));
		v.src = this.url;
		URL.revokeObjectURL(old);
		await open;
		this.aout = null;
		this.vout = null;
		this.flac?.destroy();
		this.flac = null;
		this.setDuration();
		if (vt) {
			const muxer = createMuxer(vt);
			if (!muxer) throw new Error('video muxer');
			this.vout = new Output(this.ms.addSourceBuffer(muxer.mime), muxer, vt);
			await this.vout.append(muxer.init());
		}
		if (this.audioTrack) await this.startPcm(this.audioTrack);
		v.currentTime = at;
		this.seekTo = at;
		this.poke();
		const ready = new Promise((r) => v.addEventListener('canplay', r, { once: true }));
		await Promise.race([ready, new Promise((r) => setTimeout(r, 8000))]);
		this.swapping = false;
		v.play().catch(() => {});
	}

	// ---------- Tracks ----------

	/** Switches to another audio track of the file in place. False when there is none. */
	async switchAudio(language: string, name?: string): Promise<boolean> {
		const list = this.demux.tracks.filter((t) => t.kind === 'audio');
		const pick = list.find((t) => t.language === language && (!name || t.name === name)) ?? list.find((t) => t.language === language);
		if (!pick) return false;
		if (pick === this.audioTrack || pick.id === this.audioTrack?.id) return true;
		// Reading stops and what is on its way is dropped, so nothing of either track reaches the
		// SourceBuffer while it changes.
		this.switching = true;
		this.gen++;
		try {
			await this.flacChain;
			await this.aout?.idle();
			await this.changeAudio(pick);
		} finally {
			this.switching = false;
		}
		this.seekTo = this.video.currentTime;
		this.gen++;
		this.poke();
		return true;
	}

	private async changeAudio(pick: Track) {
		this.audioTrack = pick;
		await this.selectTracks();
		const t = this.audioTrack!;
		if (this.pcm) {
			this.pcm.destroy();
			this.pcm = null;
			await this.startPcm(t);
		} else if (this.aout) {
			const o = this.aout;
			const wasFlac = this.flac != null;
			this.flac?.destroy();
			this.flac = null;
			let changed = false;
			const said: string[] = [];
			// A MediaSource whose file was read to the end is 'ended': abort() throws there, and the
			// first append opens it again.
			const abort = () => this.ms.readyState === 'open' && o.sb.abort();
			if ('changeType' in o.sb) {
				for (const mime of mimeChoices(t)) {
					if (refusedAudio.has(mime)) continue;
					const muxer = createMuxer(t, undefined, mime)!;
					try {
						await o.idle();
						abort();
						o.sb.changeType(mime);
						o.muxer = muxer;
						await o.append(muxer.init());
						changed = true;
						break;
					} catch (e) {
						// Only the TV saying no to the form rules it out for later loads.
						if ((e as DOMException)?.name !== 'InvalidStateError') refusedAudio.add(mime);
						said.push(`${mime} refused: ${errText(e)}`);
					}
				}
				// No MediaSource form: decoded here and back into the same SourceBuffer as FLAC.
				if (!changed && decoderFor(t) && this.canFlac()) {
					try {
						await o.idle();
						abort();
						if (!wasFlac) o.sb.changeType(FLAC_MIME);
						o.muxer = new Mp4Muxer(flacTrack(t.sampleRate || 48000), FLAC_MIME);
						this.flacRate = null;
						this.flac = await FlacAudio.create(this.o.base, t, this.video.currentTime, (r, frames) => this.onFlac(r, frames));
					} catch (e) {
						said.push(`${FLAC_MIME}: ${errText(e)}`);
					}
				}
			} else said.push('no changeType');
			if (!changed) {
				this.audioWhy = `audio switch: ${said.join('; ') || 'no MediaSource form'} (${trackSetup(t)})`;
				console.info('audio switch to WebAssembly:', this.audioWhy);
			}
			if (!changed && !this.flac) {
				const sb = o.sb;
				this.aout = null;
				await new Promise((r) => (sb.updating ? sb.addEventListener('updateend', r, { once: true }) : r(null)));
				this.ms.removeSourceBuffer(sb);
				await this.startPcm(t);
			} else {
				for (const [s, e] of o.ranges()) await o.remove(s, e);
			}
		}
	}

	/** Shows the file's subtitle track `index` (counted in its subtitle tracks), or none. */
	async showText(index: number | null) {
		const list = this.demux.tracks.filter((t) => t.kind === 'text');
		const pick = index == null ? null : (list[index] ?? null);
		if (pick?.id === this.textTrack?.id) return;
		this.textTrack = pick;
		await this.subs.show(pick, () => this.attachments());
		await this.selectTracks();
		if (pick) {
			// Read again from the playhead so the cues before the read position come too.
			this.seekTo = this.video.currentTime;
			this.gen++;
			this.poke();
		}
	}

	/** Plain cues for typeset subtitles that fell behind. */
	async plainText() {
		if (!this.textTrack || !/^(ass|ssa)$/.test(this.textTrack.codec)) return;
		const t = this.textTrack;
		this.subs.noTypeset();
		await this.subs.show(t, () => this.attachments());
		this.seekTo = this.video.currentTime;
		this.gen++;
		this.poke();
	}

	// ---------- Errors and the end ----------

	private fail(e: unknown) {
		if (this.failed || this.closed) return;
		this.failed = true;
		let r: Report;
		if (e instanceof Refused) r = e.report;
		else if (e instanceof FetchError) {
			const manifest = e.what === 'playlist' || e.what === 'init';
			r = report(manifest ? 'manifest' : 'network', e.message, { fetched: e.what, status: e.status, lost: e.status === 0 });
		} else r = report('unknown', `unpacking: ${errText(e)}`);
		console.warn('unpacker', r.detail);
		this.o.report(r);
		// CAF hears it as a media error and goes idle.
		try {
			if (this.ms.readyState === 'open') this.ms.endOfStream(r.part === 'network' || r.part === 'manifest' ? 'network' : 'decode');
		} catch {
			// already closed
		}
	}

	destroy() {
		if (this.closed) return;
		this.closed = true;
		this.gen++;
		this.poke();
		clearInterval(this.timer);
		clearInterval(this.traceTimer);
		for (const [target, type, fn, capture] of this.listeners) target.removeEventListener(type, fn, capture);
		this.listeners = [];
		this.pcm?.destroy();
		this.pcm = null;
		this.flac?.destroy();
		this.flac = null;
		const early = this.early;
		this.early = null;
		if (early) void early.made.then((p) => p.destroy(), () => {});
		this.subs.destroy();
		this.demux.close();
		URL.revokeObjectURL(this.url);
	}
}

function errText(e: unknown) {
	return e instanceof Error ? `${e.name === 'Error' ? '' : `${e.name}: `}${e.message}` : String(e);
}

export { LeaveToCaf };
