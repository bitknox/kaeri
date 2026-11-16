// A stream cut into segments (HLS, DASH): one rendition per track, read side by side in time
// order. Each segment is fetched whole and unpacked as TS, fMP4 or WebVTT.

import { utf8 } from './bytes';
import { initSegment, mediaSegment, type Mp4Track } from './mp4';
import { FetchError, bytesOf } from './source';
import { TsParser } from './ts';
import type { Demuxer, Sample, Track } from './types';

export interface Segment {
	url: string;
	range?: [number, number];
	/** Seconds on the presentation timeline. */
	start: number;
	duration: number;
	/** HLS discontinuity sequence: times restart, so the clock is measured again. */
	epoch: number;
	init?: { url: string; range?: [number, number] };
	/** DASH: seconds added to the segment's own times. Undefined: measure from the first segment. */
	offset?: number;
}

export interface Rendition {
	kind: Track['kind'];
	segments: Segment[];
	format: 'ts' | 'mp4' | 'vtt';
	language?: string;
	name?: string;
	default?: boolean;
	forced?: boolean;
	/** Codec strings the playlist declared. */
	codecs?: string[];
	bandwidth?: number;
	width?: number;
	height?: number;
	/** A TS or fMP4 rendition that also carries audio (HLS variants without audio groups). */
	muxed?: boolean;
}

interface Cursor {
	r: Rendition;
	/** Track id this rendition's samples get. */
	id: number;
	next: number;
	/** Parsed init segment, per init URL. */
	inits: Map<string, Map<number, Mp4Track>>;
	ts: TsParser | null;
	/** fMP4 or TS track id inside the segments, mapped to `id`. */
	inner: Map<number, number>;
	/** TS: the PIDs of a muxed variant's audio. */
	audioPid?: number;
	/** TS: the segment the parser read last, -1 after a seek. */
	last: number;
}

/**
 * How hard a segment is tried before it is skipped, and how many missed in a row mean the stream
 * is gone. Six tries over about twelve seconds stay inside the half minute read ahead.
 */
export const patience = { retries: 5, backoff: 500, timeout: 20_000, gone: 3 };

const VIDEO_ID = 1;
const AUDIO_BASE = 100;
const TEXT_BASE = 200;

function codecFromString(c: string | undefined): string | null {
	if (!c) return null;
	if (/^avc[13]/.test(c)) return 'h264';
	if (/^(hvc1|hev1|dvh1|dvhe)/.test(c)) return 'hevc';
	if (/^av01/.test(c)) return 'av1';
	if (/^vp0?9/.test(c)) return 'vp9';
	if (/^mp4a\.40/.test(c)) return 'aac';
	if (/^mp4a\.(69|6b)/i.test(c)) return 'mp3';
	if (c === 'ac-3') return 'ac3';
	if (c === 'ec-3') return 'eac3';
	if (/^dts/.test(c)) return 'dts';
	if (c === 'opus') return 'opus';
	if (c === 'flac' || c === 'fLaC') return 'flac';
	if (c === 'wvtt') return 'webvtt';
	return null;
}

export class SegmentedStream implements Demuxer {
	tracks: Track[] = [];
	duration = NaN;
	private cursors: Cursor[] = [];
	private active = new Set<number>();
	/** Seconds subtracted from media times, per epoch (HLS). */
	private origins = new Map<number, number>();
	private abort = new AbortController();
	/** Aborts the segment being read; a seek starts a new one. */
	private reading = new AbortController();
	private seekTime = 0;
	/** Per rendition, the segments read in a row that never came. */
	private missed = new Map<number, number>();
	/** Spans whose segments were skipped, in seconds on the presentation timeline. */
	readonly skipped: [number, number][] = [];

	constructor(
		private readonly video: Rendition | null,
		private readonly audio: Rendition[],
		private readonly text: Rendition[],
		duration: number,
	) {
		this.duration = duration;
	}

	async open() {
		if (this.video) {
			const c = this.cursor(this.video, VIDEO_ID);
			this.cursors.push(c);
			this.tracks.push(...(await this.probe(c)));
		}
		this.audio.forEach((r, i) => {
			const c = this.cursor(r, AUDIO_BASE + i);
			this.cursors.push(c);
			this.tracks.push({
				id: c.id,
				kind: 'audio',
				codec: codecFromString(r.codecs?.find((x) => codecFromString(x) && !/^(avc|hvc|hev|av01|vp)/.test(x))) ?? 'aac',
				language: r.language,
				name: r.name,
				default: r.default,
			});
		});
		this.text.forEach((r, i) => {
			const c = this.cursor(r, TEXT_BASE + i);
			this.cursors.push(c);
			this.tracks.push({ id: c.id, kind: 'text', codec: 'webvtt', language: r.language, name: r.name, default: r.default, forced: r.forced });
		});
		this.active = new Set(this.tracks.map((t) => t.id));
	}

	private cursor(r: Rendition, id: number): Cursor {
		return { r, id, next: 0, inits: new Map(), ts: r.format === 'ts' ? new TsParser() : null, inner: new Map(), last: -1 };
	}

	/** Reads a rendition's first segment (or init) to learn its tracks and their setup. */
	private async probe(c: Cursor): Promise<Track[]> {
		const seg = c.r.segments[0];
		if (!seg) return [];
		if (c.r.format === 'mp4') {
			const tracks = await this.init(c, seg);
			const out: Track[] = [];
			for (const t of tracks.values()) {
				if (t.kind === 'video' && c.r.kind === 'video' && !c.inner.has(t.id)) {
					c.inner.set(t.id, VIDEO_ID);
					out.push({ ...t, id: VIDEO_ID });
				} else if (t.kind === 'audio' && (c.r.kind === 'audio' || c.r.muxed) && ![...c.inner.values()].some((v) => v >= AUDIO_BASE)) {
					const id = c.r.kind === 'audio' ? c.id : AUDIO_BASE + 99;
					c.inner.set(t.id, id);
					out.push({ ...t, id, language: c.r.language ?? t.language, name: c.r.name ?? t.name });
				}
			}
			if (seg.offset == null) await this.measure(c, seg, out);
			return out;
		}
		// TS: parse the first segment.
		const p = new TsParser();
		const samples = [...p.push(await this.fetch(seg)), ...p.flush()];
		const out: Track[] = [];
		const v = p.tracks.find((t) => t.kind === 'video');
		const a = p.tracks.find((t) => t.kind === 'audio');
		if (v && c.r.kind === 'video') {
			c.inner.set(v.id, VIDEO_ID);
			out.push({ ...v, id: VIDEO_ID });
		}
		if (a && (c.r.kind === 'audio' || c.r.muxed !== false)) {
			const id = c.r.kind === 'audio' ? c.id : AUDIO_BASE + 99;
			c.inner.set(a.id, id);
			out.push({ ...a, id, language: c.r.language ?? a.language, name: c.r.name ?? a.name });
		}
		if (!this.origins.has(seg.epoch)) {
			const first = samples.find((s) => s.track === v?.id) ?? samples[0];
			if (first) this.origins.set(seg.epoch, Math.min(first.pts, first.dts) - seg.start);
		}
		return out;
	}

	private async measure(c: Cursor, seg: Segment, tracks: Track[]) {
		if (this.origins.has(seg.epoch)) return;
		const map = c.inits.get(seg.init?.url ?? '') ?? new Map();
		const samples = mediaSegment(await this.fetch(seg), map);
		const v = samples.find((s) => tracks.some((t) => t.kind === 'video' && c.inner.get(s.track) === t.id)) ?? samples[0];
		if (v) this.origins.set(seg.epoch, Math.min(v.pts, v.dts) - seg.start);
	}

	private async init(c: Cursor, seg: Segment): Promise<Map<number, Mp4Track>> {
		const key = seg.init?.url ?? '';
		const have = c.inits.get(key);
		if (have) return have;
		if (!seg.init) {
			const empty = new Map<number, Mp4Track>();
			c.inits.set(key, empty);
			return empty;
		}
		const b = await bytesOf(seg.init.url, { range: seg.init.range, signal: this.abort.signal, what: 'init' });
		const m = initSegment(b);
		const map = new Map(m.tracks.map((t) => [t.id, t]));
		c.inits.set(key, map);
		return map;
	}

	private fetch(seg: Segment) {
		return bytesOf(seg.url, { range: seg.range, signal: this.abort.signal, what: 'segment' });
	}

	/**
	 * A segment read during playback. One that still fails after its retries is skipped and
	 * playback steps over its span; several in a row mean the stream is gone, and that error
	 * ends the session.
	 */
	private async fetchOrSkip(seg: Segment, c: Cursor): Promise<Uint8Array | null> {
		const { retries, backoff, timeout, gone } = patience;
		const id = c.id;
		try {
			const b = await bytesOf(seg.url, { range: seg.range, signal: this.reading.signal, what: 'segment', retries, backoff, timeout });
			this.missed.delete(id);
			return b;
		} catch (e) {
			if (!(e instanceof FetchError) || this.reading.signal.aborted) throw e;
			// Missing subtitle cues leave the picture and sound alone.
			if (c.r.kind === 'text') return null;
			const missed = (this.missed.get(id) ?? 0) + 1;
			this.missed.set(id, missed);
			if (missed >= gone) throw e;
			console.warn('segment skipped', e.message, seg.start.toFixed(1));
			const end = seg.start + seg.duration;
			if (!this.skipped.some(([a, b]) => a === seg.start && b === end)) this.skipped.push([seg.start, end]);
			return null;
		}
	}

	/** Stops the segment being read, for a seek that wants reading elsewhere now. */
	interrupt() {
		this.reading.abort();
		this.reading = new AbortController();
	}

	/** Loads the setup of the audio renditions about to be read. */
	async select(ids: number[]) {
		this.active = new Set(ids);
		for (const c of this.cursors) {
			if (!this.active.has(c.id) || c.r.kind !== 'audio' || c.inner.size) continue;
			const probed = await this.probe(c);
			const t = probed.find((x) => x.kind === 'audio');
			const i = this.tracks.findIndex((x) => x.id === c.id);
			if (t && i >= 0) this.tracks[i] = { ...t, id: c.id };
		}
	}

	/** Whether a track's setup is known (its rendition was read). */
	ready(id: number) {
		const c = this.cursors.find((x) => x.id === id);
		return !c || c.r.kind !== 'audio' || c.inner.size > 0;
	}

	async seek(time: number): Promise<number> {
		this.seekTime = time;
		let start = time;
		for (const c of this.cursors) {
			const segs = c.r.segments;
			let i = segs.findIndex((s) => s.start + s.duration > time + 1e-3);
			if (i < 0) i = segs.length;
			c.next = i;
			c.ts?.reset();
			c.last = -1;
			if (c.r.kind === 'video' && segs[i]) start = segs[i].start;
		}
		return start;
	}

	async read(): Promise<Sample[] | null> {
		// The wanted rendition furthest behind goes next.
		let pick: Cursor | null = null;
		for (const c of this.cursors) {
			if (!this.active.has(c.id) && ![...c.inner.values()].some((v) => this.active.has(v))) continue;
			const s = c.r.segments[c.next];
			if (!s) continue;
			if (!pick || s.start < pick.r.segments[pick.next].start) pick = c;
		}
		if (!pick) return null;
		const index = pick.next++;
		const seg = pick.r.segments[index];
		const b = await this.fetchOrSkip(seg, pick);
		// A skipped TS segment breaks the run: the next one starts its parser over.
		if (!b) return [];
		if (pick.r.format === 'vtt') return this.vtt(utf8(b), seg, pick.id);
		if (pick.r.format === 'mp4') {
			const map = await this.init(pick, seg);
			if (!pick.inner.size) for (const t of map.values()) if (t.kind === pick.r.kind) pick.inner.set(t.id, pick.id);
			return this.place(mediaSegment(b, map), seg, pick);
		}
		// A PES packet, and an audio frame inside it, may run on from one segment into the next:
		// segmenters cut at video keyframes, not between audio packets. Segments read in a row go
		// through one parser, so what the last one left open completes here. After a seek or a
		// discontinuity it starts over, and what was open belongs to the segment before.
		const ts = pick.ts!;
		const prev = pick.last >= 0 ? pick.r.segments[pick.last] : null;
		let tail: Sample[] = [];
		if (prev && pick.last === index - 1 && prev.epoch === seg.epoch) ts.nextSegment();
		else {
			if (prev) tail = this.place(ts.flush(), prev, pick);
			ts.reset();
		}
		pick.last = index;
		const samples = ts.push(b);
		// Nothing follows the last segment to complete what is open.
		if (index === pick.r.segments.length - 1) samples.push(...ts.flush());
		return [...tail, ...this.place(samples, seg, pick)];
	}

	/** A segment's samples on the presentation timeline, under the stream's track ids. */
	private place(samples: Sample[], seg: Segment, pick: Cursor): Sample[] {
		let origin: number;
		if (seg.offset != null) origin = -seg.offset;
		else {
			if (!this.origins.has(seg.epoch)) {
				const first = samples[0];
				this.origins.set(seg.epoch, first ? Math.min(first.pts, first.dts) - seg.start : 0);
			}
			origin = this.origins.get(seg.epoch)!;
		}
		const out: Sample[] = [];
		for (const s of samples) {
			const id = pick.inner.get(s.track);
			if (id == null || !this.active.has(id)) continue;
			s.track = id;
			s.pts -= origin;
			s.dts -= origin;
			out.push(s);
		}
		return out;
	}

	/** A WebVTT segment's cues as samples (text, with pts and duration). */
	private vtt(text: string, seg: Segment, id: number): Sample[] {
		let shift = 0;
		const map = text.match(/X-TIMESTAMP-MAP=([^\n]+)/);
		if (map) {
			const mpegts = Number(map[1].match(/MPEGTS:(\d+)/)?.[1] ?? 0) / 90000;
			const local = clock(map[1].match(/LOCAL:([\d:.]+)/)?.[1] ?? '0');
			shift = mpegts - local - (this.origins.get(seg.epoch) ?? 0);
		}
		const out: Sample[] = [];
		const enc = new TextEncoder();
		for (const block of text.replace(/\r/g, '').split(/\n\n+/)) {
			const m = block.match(/([\d:.]+)\s+-->\s+([\d:.]+)([^\n]*)\n([\s\S]*)/);
			if (!m) continue;
			const a = clock(m[1]) + shift;
			const b = clock(m[2]) + shift;
			out.push({ track: id, pts: a, dts: a, duration: b - a, key: true, data: enc.encode(m[4].trim()) });
		}
		return out;
	}

	close() {
		this.abort.abort();
		this.reading.abort();
	}
}

/** Seconds from a WebVTT or SRT clock (hh:mm:ss.mmm or mm:ss.mmm). */
export function clock(s: string): number {
	const parts = s.replace(',', '.').split(':').map(Number);
	return parts.reduce((acc, v) => acc * 60 + v, 0);
}

export { FetchError };
