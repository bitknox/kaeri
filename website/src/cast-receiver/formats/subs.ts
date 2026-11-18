// Subtitles carried inside the container: text (SubRip, WebVTT, MP4 timed text, HLS WebVTT) as
// cues on a text track of the video element, so they look like the receiver's other plain
// subtitles; images (PGS, VobSub) on a canvas over the picture; ASS through the receiver's
// typeset renderer when it is there, else as plain cues.

import { PgsDecoder, vobsubPacket, vobsubSetup, type Display, type VobSubSetup } from './bitmap-subs';
import type { Attachment, Sample, Track } from './types';

/** The typeset renderer (JASSUB) as the receiver exposes it for subtitles inside the file. */
export interface TypesetSink {
	/** Starts typeset subtitles with the script's header and the file's fonts. */
	start(header: string, fonts: Uint8Array[]): Promise<void>;
	/** One Matroska ASS block: "ReadOrder, Layer, Style, ..., Text", times in seconds. */
	chunk(text: string, start: number, duration: number): void;
	stop(): void;
}

const FONT = /\.(ttf|otf|ttc|woff2?)$/i;

export class Subtitles {
	private track: Track | null = null;
	private text: TextTrack | null = null;
	private seen = new Set<string>();
	private canvas: HTMLCanvasElement | null = null;
	private displays: Display[] = [];
	private bitmaps = new Map<Display, ImageBitmap[]>();
	private drawn: Display | null | undefined = undefined;
	private pgs: PgsDecoder | null = null;
	private vobsub: VobSubSetup | null = null;
	private raf = 0;
	private typeset = false;
	/** Typeset rendering fell behind: ASS shows as plain cues from now on. */
	private typesetOff = false;
	/** Samples that arrive while the typeset renderer starts. */
	private pending: Sample[] | null = null;
	/** Cues of a text track whose end the file didn't give: closed by the next cue. */
	private open: VTTCue | null = null;

	constructor(
		private readonly video: HTMLVideoElement,
		private readonly sink: TypesetSink | null = null,
	) {}

	get current() {
		return this.track;
	}

	/** Shows `track` (or nothing). Samples already pushed for another track are dropped. */
	async show(track: Track | null, attachments: () => Promise<Attachment[]>) {
		this.clear();
		this.track = track;
		if (!track) return;
		if (track.codec === 'pgs') this.pgs = new PgsDecoder();
		else if (track.codec === 'vobsub') this.vobsub = vobsubSetup(track.config);
		if (track.codec === 'pgs' || track.codec === 'vobsub') {
			this.startCanvas();
			return;
		}
		if ((track.codec === 'ass' || track.codec === 'ssa') && this.sink && !this.typesetOff) {
			this.pending = [];
			try {
				const fonts = (await attachments()).filter((a) => FONT.test(a.name) || /font/i.test(a.mime)).map((a) => a.data);
				await this.sink.start(new TextDecoder().decode(track.config ?? new Uint8Array(0)), fonts);
				if (this.track !== track) return;
				this.typeset = true;
			} catch {
				// Plain cues instead.
				if (this.track !== track) return;
				this.textTrack();
			}
			const held = this.pending ?? [];
			this.pending = null;
			this.push(held);
			return;
		}
		this.textTrack();
	}

	/** ASS from now on as plain cues (the typeset renderer fell behind). */
	noTypeset() {
		this.typesetOff = true;
	}

	private textTrack() {
		const t = this.video.addTextTrack('subtitles', 'kaeri', this.track?.language ?? '');
		t.mode = 'showing';
		this.text = t;
	}

	push(samples: Sample[]) {
		const t = this.track;
		if (!t) return;
		if (this.pending) {
			this.pending.push(...samples.filter((s) => s.track === t.id));
			return;
		}
		for (const s of samples) {
			if (s.track !== t.id) continue;
			const key = `${s.pts.toFixed(3)}:${s.data.length}`;
			if (this.seen.has(key)) continue;
			this.seen.add(key);
			if (this.pgs || this.vobsub) {
				// A packet with no picture clears the screen; the block's duration ends a display.
				const d = this.pgs ? this.pgs.push(s.data, s.pts) : vobsubPacket(s.data, s.pts, this.vobsub!);
				const shown = d ?? (this.vobsub ? { start: s.pts, end: null, width: 0, height: 0, pictures: [] } : null);
				if (shown && shown.end == null && s.duration > 0) shown.end = s.pts + s.duration;
				this.addDisplay(shown);
			} else if (this.typeset && this.sink) this.sink.chunk(new TextDecoder().decode(s.data), s.pts, s.duration || 5);
			else this.cue(t, s);
		}
		if (this.text && this.text.mode !== 'showing') this.text.mode = 'showing';
	}

	private cue(t: Track, s: Sample) {
		let text: string;
		if (t.codec === 'tx3g') {
			const len = s.data.length >= 2 ? (s.data[0] << 8) | s.data[1] : 0;
			text = new TextDecoder().decode(s.data.subarray(2, 2 + len));
		} else text = new TextDecoder().decode(s.data);
		let top = false;
		if (t.codec === 'ass' || t.codec === 'ssa') {
			// ReadOrder, Layer, Style, Name, MarginL, MarginR, MarginV, Effect, Text
			const parts = text.split(',');
			text = parts.slice(8).join(',');
			top = /\{[^}]*\\an?[789][^}]*\}/.test(text);
			text = text.replace(/\{[^}]*\}/g, '').replace(/\\N/gi, '\n').replace(/\\h/g, ' ');
		} else if (t.codec === 'srt') {
			top = /\{\\an?[789]\}/.test(text);
			text = text.replace(/\{\\[^}]*\}/g, '').replace(/<\/?font[^>]*>/gi, '');
		}
		if (!text.trim()) return;
		if (this.open && this.open.endTime > s.pts) this.open.endTime = Math.max(this.open.startTime + 0.1, s.pts);
		const end = s.duration > 0 ? s.pts + s.duration : s.pts + 5;
		const cue = new VTTCue(s.pts, end, text);
		if (top) cue.line = 0;
		this.text?.addCue(cue);
		this.open = s.duration > 0 ? null : cue;
	}

	private addDisplay(d: Display | null) {
		if (!d) return;
		this.displays.push(d);
		this.displays.sort((a, b) => a.start - b.start);
		void Promise.all(
			d.pictures.map((p) => createImageBitmap(new ImageData(p.rgba as Uint8ClampedArray<ArrayBuffer>, p.width, p.height))),
		).then((list) => {
			this.bitmaps.set(d, list);
			for (const p of d.pictures) p.rgba = new Uint8ClampedArray(0);
			this.drawn = undefined;
		});
		// Keep a minute of displays behind the playhead.
		const keepFrom = this.video.currentTime - 60;
		while (this.displays.length > 1 && (this.displays[0].end ?? this.displays[1].start) < keepFrom) {
			const old = this.displays.shift()!;
			this.bitmaps.get(old)?.forEach((b) => b.close());
			this.bitmaps.delete(old);
		}
	}

	private startCanvas() {
		if (!this.canvas) {
			const c = document.createElement('canvas');
			c.id = 'image-subs';
			c.setAttribute('aria-hidden', 'true');
			c.style.cssText = 'position:absolute;inset:0;width:100%;height:100%;pointer-events:none';
			this.video.insertAdjacentElement('afterend', c);
			this.canvas = c;
		}
		const tick = () => {
			this.draw();
			this.raf = requestAnimationFrame(tick);
		};
		this.raf = requestAnimationFrame(tick);
	}

	private draw() {
		const c = this.canvas;
		if (!c) return;
		const t = this.video.currentTime;
		// The last display started; it lasts until its own end or the next display.
		let d: Display | null = null;
		const list_ = this.displays;
		for (let i = 0; i < list_.length && list_[i].start <= t; i++) {
			const x = list_[i];
			const end = x.end ?? list_[i + 1]?.start ?? Infinity;
			d = end > t && x.pictures.length ? x : null;
		}
		const w = Math.round(c.clientWidth * devicePixelRatio);
		const h = Math.round(c.clientHeight * devicePixelRatio);
		if (d === this.drawn && c.width === w && c.height === h) return;
		const list = d ? this.bitmaps.get(d) : undefined;
		if (d && !list) return; // still decoding
		this.drawn = d;
		if (c.width !== w || c.height !== h) {
			c.width = w;
			c.height = h;
		}
		const g = c.getContext('2d')!;
		g.clearRect(0, 0, w, h);
		if (!d || !list) return;
		// The video's picture inside the element (object-fit: contain).
		const vw = this.video.videoWidth || d.width;
		const vh = this.video.videoHeight || d.height;
		const scale = Math.min(w / vw, h / vh);
		const px = (w - vw * scale) / 2;
		const py = (h - vh * scale) / 2;
		const sx = (vw * scale) / d.width;
		const sy = (vh * scale) / d.height;
		d.pictures.forEach((p, i) => g.drawImage(list[i], px + p.x * sx, py + p.y * sy, p.width * sx, p.height * sy));
	}

	/** After a seek: pushed samples come again, so nothing is lost by clearing state. */
	seek() {
		this.seen.clear();
		this.open = null;
	}

	private clear() {
		cancelAnimationFrame(this.raf);
		this.raf = 0;
		if (this.text) {
			this.text.mode = 'disabled';
			for (const cue of Array.from(this.text.cues ?? [])) this.text.removeCue(cue);
			this.text = null;
		}
		if (this.typeset || this.pending) this.sink?.stop();
		this.typeset = false;
		this.pending = null;
		for (const list of this.bitmaps.values()) list.forEach((b) => b.close());
		this.bitmaps.clear();
		this.displays = [];
		this.drawn = undefined;
		this.pgs = null;
		this.vobsub = null;
		this.seen.clear();
		this.open = null;
		if (this.canvas) this.canvas.getContext('2d')?.clearRect(0, 0, this.canvas.width, this.canvas.height);
	}

	destroy() {
		this.clear();
		this.canvas?.remove();
		this.canvas = null;
		this.track = null;
	}
}

/** A side-loaded .sup file (PGS) as one sample per display set, for `Subtitles.push`. */
export function supSamples(b: Uint8Array, track: number): Sample[] {
	const out: Sample[] = [];
	let at = 0;
	let start = -1;
	let pts = 0;
	while (at + 13 <= b.length && b[at] === 0x50 && b[at + 1] === 0x47) {
		const size = (b[at + 11] << 8) | b[at + 12];
		const type = b[at + 10];
		if (start < 0) {
			start = at;
			pts = (((b[at + 2] << 24) >>> 0) + (b[at + 3] << 16) + (b[at + 4] << 8) + b[at + 5]) / 90000;
		}
		at += 13 + size;
		if (type === 0x80) {
			out.push({ track, pts, dts: pts, duration: 0, key: true, data: b.subarray(start, at) });
			start = -1;
		}
	}
	return out;
}
