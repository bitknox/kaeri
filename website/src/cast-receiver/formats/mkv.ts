// Matroska and WebM: tracks, attachments and blocks from a file read over HTTP ranges. Seeking
// uses the Cues when the file has them, else a bisection over the clusters' timecodes.

import { ascii, utf8 } from './bytes';
import type { ByteSource } from './source';
import type { Attachment, Demuxer, Sample, Track } from './types';

const ID = {
	EBML: 0x1a45dfa3,
	Segment: 0x18538067,
	SeekHead: 0x114d9b74,
	Seek: 0x4dbb,
	SeekID: 0x53ab,
	SeekPosition: 0x53ac,
	Info: 0x1549a966,
	TimecodeScale: 0x2ad7b1,
	Duration: 0x4489,
	Tracks: 0x1654ae6b,
	TrackEntry: 0xae,
	TrackNumber: 0xd7,
	TrackType: 0x83,
	CodecID: 0x86,
	CodecPrivate: 0x63a2,
	Language: 0x22b59c,
	LanguageBCP47: 0x22b59d,
	Name: 0x536e,
	FlagDefault: 0x88,
	FlagForced: 0x55aa,
	FlagEnabled: 0xb9,
	DefaultDuration: 0x23e383,
	Video: 0xe0,
	PixelWidth: 0xb0,
	PixelHeight: 0xba,
	Audio: 0xe1,
	SamplingFrequency: 0xb5,
	OutputSamplingFrequency: 0x78b5,
	Channels: 0x9f,
	BitDepth: 0x6264,
	ContentEncodings: 0x6d80,
	ContentEncoding: 0x6240,
	ContentCompression: 0x5034,
	ContentCompAlgo: 0x4254,
	ContentCompSettings: 0x4255,
	Cluster: 0x1f43b675,
	Timecode: 0xe7,
	SimpleBlock: 0xa3,
	BlockGroup: 0xa0,
	Block: 0xa1,
	BlockDuration: 0x9b,
	ReferenceBlock: 0xfb,
	Cues: 0x1c53bb6b,
	CuePoint: 0xbb,
	CueTime: 0xb3,
	CueTrackPositions: 0xb7,
	CueTrack: 0xf7,
	CueClusterPosition: 0xf1,
	Attachments: 0x1941a469,
	AttachedFile: 0x61a7,
	FileName: 0x466e,
	FileMimeType: 0x4660,
	FileData: 0x465c,
} as const;

const TOP_LEVEL = new Set<number>([
	ID.SeekHead,
	ID.Info,
	ID.Tracks,
	ID.Cluster,
	ID.Cues,
	ID.Attachments,
	0x1254c367, // Tags
	0x1043a770, // Chapters
	ID.Segment,
	ID.EBML,
]);

const UNKNOWN = -1;

interface Header {
	id: number;
	/** Bytes of data; UNKNOWN for an open-ended element. */
	size: number;
	/** Offset of the data. */
	data: number;
	/** Bytes of the ID and size. */
	length: number;
}

/** Reads an EBML header from bytes at `at`. Null when the bytes run out or aren't EBML. */
export function header(b: Uint8Array, at: number, base = 0): Header | null {
	const first = b[at];
	if (first === undefined || first === 0) return null;
	let idLen = 1;
	while (idLen <= 4 && !(first & (0x80 >> (idLen - 1)))) idLen++;
	if (idLen > 4 || at + idLen > b.length) return null;
	let id = 0;
	for (let i = 0; i < idLen; i++) id = id * 256 + b[at + i];
	const s = at + idLen;
	const sFirst = b[s];
	if (sFirst === undefined || sFirst === 0) return null;
	let sLen = 1;
	while (sLen <= 8 && !(sFirst & (0x80 >> (sLen - 1)))) sLen++;
	if (sLen > 8 || s + sLen > b.length) return null;
	let size = sFirst & (0xff >> sLen);
	let allOnes = size === 0xff >> sLen;
	for (let i = 1; i < sLen; i++) {
		size = size * 256 + b[s + i];
		if (b[s + i] !== 0xff) allOnes = false;
	}
	return { id, size: allOnes ? UNKNOWN : size, data: base + s + sLen, length: idLen + sLen };
}

function uint(b: Uint8Array): number {
	let v = 0;
	for (const x of b) v = v * 256 + x;
	return v;
}
function float(b: Uint8Array): number {
	const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
	return b.length === 4 ? v.getFloat32(0) : b.length === 8 ? v.getFloat64(0) : 0;
}

/** The children of an element whose bytes are all in memory. */
function* children(b: Uint8Array, from = 0, to = b.length): Generator<{ id: number; body: Uint8Array }> {
	let at = from;
	while (at < to) {
		const h = header(b, at);
		if (!h) return;
		const end = h.size === UNKNOWN ? to : Math.min(to, h.data + h.size);
		yield { id: h.id, body: b.subarray(h.data, end) };
		at = end;
	}
}

/** Lace sizes of a block's frames (Xiph, fixed or EBML lacing). */
function unlace(b: Uint8Array, at: number, lacing: number): Uint8Array[] {
	if (lacing === 0) return [b.subarray(at)];
	const count = b[at++] + 1;
	const sizes: number[] = [];
	if (lacing === 1) {
		for (let i = 0; i < count - 1; i++) {
			let s = 0;
			let x;
			do {
				x = b[at++];
				s += x;
			} while (x === 255);
			sizes.push(s);
		}
	} else if (lacing === 3) {
		const read = () => {
			const first = b[at];
			let len = 1;
			while (len <= 8 && !(first & (0x80 >> (len - 1)))) len++;
			let v = first & (0xff >> len);
			for (let i = 1; i < len; i++) v = v * 256 + b[at + i];
			at += len;
			return { v, len };
		};
		let s = read().v;
		sizes.push(s);
		for (let i = 1; i < count - 1; i++) {
			const { v, len } = read();
			s += v - (2 ** (7 * len - 1) - 1);
			sizes.push(s);
		}
	} else if (lacing === 2) {
		const each = Math.floor((b.length - at) / count);
		for (let i = 0; i < count - 1; i++) sizes.push(each);
	}
	const out: Uint8Array[] = [];
	for (const s of sizes) {
		out.push(b.subarray(at, at + s));
		at += s;
	}
	out.push(b.subarray(at));
	return out;
}

const VIDEO_CODECS: Record<string, string> = {
	'V_MPEG4/ISO/AVC': 'h264',
	'V_MPEGH/ISO/HEVC': 'hevc',
	V_VP8: 'vp8',
	V_VP9: 'vp9',
	V_AV1: 'av1',
	V_MPEG2: 'mpeg2',
	'V_MPEG4/ISO/ASP': 'mpeg4',
};

function audioCodec(id: string, bitDepth?: number): string {
	if (id.startsWith('A_AAC')) return 'aac';
	if (id.startsWith('A_DTS')) return 'dts';
	if (id === 'A_PCM/INT/LIT') return bitDepth === 24 ? 'pcm_s24le' : bitDepth === 32 ? 'pcm_s32le' : bitDepth === 8 ? 'pcm_u8' : 'pcm_s16le';
	if (id === 'A_PCM/INT/BIG') return bitDepth === 24 ? 'pcm_s24be' : 'pcm_s16be';
	if (id === 'A_PCM/FLOAT/IEEE') return bitDepth === 64 ? 'pcm_f64le' : 'pcm_f32le';
	return (
		{
			'A_MPEG/L3': 'mp3',
			'A_MPEG/L2': 'mp2',
			'A_MPEG/L1': 'mp1',
			A_AC3: 'ac3',
			'A_AC3/BSID9': 'ac3',
			'A_AC3/BSID10': 'ac3',
			A_EAC3: 'eac3',
			A_TRUEHD: 'truehd',
			A_MLP: 'mlp',
			A_FLAC: 'flac',
			A_OPUS: 'opus',
			A_VORBIS: 'vorbis',
			A_ALAC: 'alac',
		} as Record<string, string>
	)[id] ?? id.toLowerCase();
}

const TEXT_CODECS: Record<string, string> = {
	'S_TEXT/UTF8': 'srt',
	'S_TEXT/ASCII': 'srt',
	'S_TEXT/WEBVTT': 'webvtt',
	'D_WEBVTT/SUBTITLES': 'webvtt',
	'D_WEBVTT/CAPTIONS': 'webvtt',
	'S_TEXT/ASS': 'ass',
	'S_TEXT/SSA': 'ass',
	S_ASS: 'ass',
	S_SSA: 'ass',
	'S_HDMV/PGS': 'pgs',
	S_VOBSUB: 'vobsub',
};

interface Encoding {
	/** Header stripping: bytes taken off the front of every frame. */
	strip?: Uint8Array;
	/** zlib-compressed frames. */
	zlib?: boolean;
}

async function inflate(data: Uint8Array): Promise<Uint8Array> {
	const stream = new Blob([data as BlobPart]).stream().pipeThrough(new DecompressionStream('deflate'));
	return new Uint8Array(await new Response(stream).arrayBuffer());
}

export class Matroska implements Demuxer {
	tracks: Track[] = [];
	duration = NaN;
	private scale = 1e6; // ns per timecode tick
	private segment = 0; // offset of the Segment's data
	private segmentEnd = Infinity;
	private positions = new Map<number, number>(); // top-level ID -> offset
	private cues: { time: number; pos: number }[] | null = null;
	private attachmentsAt: number | null = null;
	private encodings = new Map<number, Encoding>();
	/** Seconds per frame by track, to time the frames of a laced block that gives no duration. */
	private frameTimes = new Map<number, number>();
	/** The last per-frame time a laced block of each track was spread over (see spread()). */
	private spreadTimes = new Map<number, number>();
	private wanted = new Set<number>();
	private pos = 0;
	private videoId: number | null = null;

	constructor(private readonly src: ByteSource) {}

	async open() {
		const head = await this.src.bytes(0, 64);
		const ebml = header(head, 0);
		if (!ebml || ebml.id !== ID.EBML) throw new Error('not Matroska');
		const seg = header(await this.src.bytes(ebml.data + ebml.size, 16), 0, ebml.data + ebml.size);
		if (!seg || seg.id !== ID.Segment) throw new Error('no Segment');
		this.segment = seg.data;
		this.segmentEnd = seg.size === UNKNOWN ? Infinity : seg.data + seg.size;
		if (Number.isFinite(this.src.size)) this.segmentEnd = Math.min(this.segmentEnd, this.src.size);

		// Walk the top level until the first Cluster, reading SeekHead, Info and Tracks.
		let at = seg.data;
		for (let guard = 0; guard < 64 && at < this.segmentEnd; guard++) {
			const h = header(await this.src.bytes(at, 16), 0, at);
			if (!h) break;
			if (h.id === ID.Cluster) {
				this.positions.set(ID.Cluster, at);
				break;
			}
			if (!this.positions.has(h.id)) this.positions.set(h.id, at);
			if (h.size === UNKNOWN) break;
			if (h.id === ID.SeekHead || h.id === ID.Info || h.id === ID.Tracks) {
				const body = await this.src.bytes(h.data, h.size);
				if (h.id === ID.SeekHead) this.seekHead(body);
				else if (h.id === ID.Info) this.info(body);
				else await this.trackList(body);
			}
			at = h.data + h.size;
		}
		// Info or Tracks after the clusters (rare), through the SeekHead.
		for (const id of [ID.Info, ID.Tracks]) {
			if (id === ID.Tracks && this.tracks.length) continue;
			if (id === ID.Info && Number.isFinite(this.duration)) continue;
			const p = this.positions.get(id);
			if (p == null || p < (this.positions.get(ID.Cluster) ?? Infinity)) continue;
			const h = header(await this.src.bytes(p, 16), 0, p);
			if (!h || h.id !== id || h.size === UNKNOWN) continue;
			const body = await this.src.bytes(h.data, h.size);
			if (id === ID.Info) this.info(body);
			else await this.trackList(body);
		}
		if (!this.tracks.length) throw new Error('no tracks');
		this.videoId = this.tracks.find((t) => t.kind === 'video')?.id ?? null;
		this.pos = this.positions.get(ID.Cluster) ?? this.segmentEnd;
		this.wanted = new Set(this.tracks.map((t) => t.id));
	}

	private seekHead(body: Uint8Array) {
		for (const s of children(body)) {
			if (s.id !== ID.Seek) continue;
			let id = 0;
			let pos = -1;
			for (const c of children(s.body)) {
				if (c.id === ID.SeekID) id = uint(c.body);
				else if (c.id === ID.SeekPosition) pos = uint(c.body);
			}
			if (id && pos >= 0 && !this.positions.has(id)) this.positions.set(id, this.segment + pos);
			if (id === ID.Attachments && pos >= 0) this.attachmentsAt = this.segment + pos;
		}
	}

	private info(body: Uint8Array) {
		let duration = NaN;
		for (const c of children(body)) {
			if (c.id === ID.TimecodeScale) this.scale = uint(c.body);
			else if (c.id === ID.Duration) duration = float(c.body);
		}
		if (Number.isFinite(duration)) this.duration = (duration * this.scale) / 1e9;
	}

	private async trackList(body: Uint8Array) {
		for (const e of children(body)) {
			if (e.id !== ID.TrackEntry) continue;
			const t: Partial<Track> & { type?: number; codecId?: string; enabled?: boolean } = { enabled: true };
			let encoding: Encoding | null = null;
			let defaultDuration = 0; // ns per frame
			let coreRate = 0;
			for (const c of children(e.body)) {
				switch (c.id) {
					case ID.TrackNumber: t.id = uint(c.body); break;
					case ID.DefaultDuration: defaultDuration = uint(c.body); break;
					case ID.TrackType: t.type = uint(c.body); break;
					case ID.CodecID: t.codecId = ascii(c.body).replace(/\0+$/, ''); break;
					case ID.CodecPrivate: t.config = c.body.slice(); break;
					case ID.Language: if (!t.language) t.language = ascii(c.body).replace(/\0+$/, ''); break;
					case ID.LanguageBCP47: t.language = ascii(c.body).replace(/\0+$/, ''); break;
					case ID.Name: t.name = utf8(c.body); break;
					case ID.FlagDefault: t.default = uint(c.body) === 1; break;
					case ID.FlagForced: t.forced = uint(c.body) === 1; break;
					case ID.FlagEnabled: t.enabled = uint(c.body) === 1; break;
					case ID.Video:
						for (const v of children(c.body)) {
							if (v.id === ID.PixelWidth) t.width = uint(v.body);
							else if (v.id === ID.PixelHeight) t.height = uint(v.body);
						}
						break;
					case ID.Audio: {
						let out = 0;
						for (const a of children(c.body)) {
							if (a.id === ID.SamplingFrequency) t.sampleRate = coreRate = float(a.body);
							else if (a.id === ID.OutputSamplingFrequency) out = float(a.body);
							else if (a.id === ID.Channels) t.channels = uint(a.body);
							else if (a.id === ID.BitDepth) t.bitDepth = uint(a.body);
						}
						// HE-AAC signals its doubled rate here.
						if (out) t.sampleRate = out;
						break;
					}
					case ID.ContentEncodings:
						encoding = this.encoding(c.body);
						break;
				}
			}
			if (t.id == null || !t.codecId || t.enabled === false) continue;
			const id = t.codecId;
			let track: Track | null = null;
			if (t.type === 1) track = { ...(t as Track), kind: 'video', codec: VIDEO_CODECS[id] ?? id.toLowerCase() };
			else if (t.type === 2) track = { ...(t as Track), kind: 'audio', codec: audioCodec(id, t.bitDepth) };
			else if ((t.type === 0x11 || id.startsWith('D_WEBVTT')) && TEXT_CODECS[id]) track = { ...(t as Track), kind: 'text', codec: TEXT_CODECS[id] };
			if (!track) continue;
			delete (track as { type?: number }).type;
			delete (track as { codecId?: string }).codecId;
			delete (track as { enabled?: boolean }).enabled;
			if (track.language === 'und') track.language = undefined;
			if (encoding) {
				this.encodings.set(track.id, encoding);
				if (encoding.zlib && track.config) track.config = await inflate(track.config);
			}
			if (track.codec === 'aac' && !track.config) track.config = aacConfigFromId(id, track);
			const frame = defaultDuration > 0 ? defaultDuration / 1e9 : track.kind === 'audio' ? codecFrameTime(track.codec, coreRate) : 0;
			if (frame > 0) this.frameTimes.set(track.id, frame);
			this.tracks.push(track);
		}
	}

	private encoding(body: Uint8Array): Encoding | null {
		for (const e of children(body)) {
			if (e.id !== ID.ContentEncoding) continue;
			for (const c of children(e.body)) {
				if (c.id !== ID.ContentCompression) continue;
				let algo = 0;
				let settings: Uint8Array | undefined;
				for (const x of children(c.body)) {
					if (x.id === ID.ContentCompAlgo) algo = uint(x.body);
					else if (x.id === ID.ContentCompSettings) settings = x.body.slice();
				}
				if (algo === 3) return { strip: settings };
				if (algo === 0) return { zlib: true };
			}
		}
		return null;
	}

	select(ids: number[]) {
		this.wanted = new Set(ids);
	}

	async attachments(): Promise<Attachment[]> {
		const at = this.attachmentsAt ?? this.positions.get(ID.Attachments);
		if (at == null) return [];
		const h = header(await this.src.bytes(at, 16), 0, at);
		if (!h || h.id !== ID.Attachments || h.size === UNKNOWN) return [];
		const body = await this.src.bytes(h.data, h.size);
		const out: Attachment[] = [];
		for (const f of children(body)) {
			if (f.id !== ID.AttachedFile) continue;
			const a: Attachment = { name: '', mime: '', data: new Uint8Array(0) };
			for (const c of children(f.body)) {
				if (c.id === ID.FileName) a.name = utf8(c.body);
				else if (c.id === ID.FileMimeType) a.mime = ascii(c.body);
				else if (c.id === ID.FileData) a.data = c.body.slice();
			}
			out.push(a);
		}
		return out;
	}

	private async loadCues() {
		if (this.cues) return this.cues;
		this.cues = [];
		const at = this.positions.get(ID.Cues);
		if (at == null) return this.cues;
		const h = header(await this.src.bytes(at, 16), 0, at);
		if (!h || h.id !== ID.Cues || h.size === UNKNOWN) return this.cues;
		const body = await this.src.bytes(h.data, h.size);
		for (const p of children(body)) {
			if (p.id !== ID.CuePoint) continue;
			let time = -1;
			let pos = -1;
			for (const c of children(p.body)) {
				if (c.id === ID.CueTime) time = uint(c.body);
				else if (c.id === ID.CueTrackPositions) {
					let track = 0;
					let cluster = -1;
					for (const x of children(c.body)) {
						if (x.id === ID.CueTrack) track = uint(x.body);
						else if (x.id === ID.CueClusterPosition) cluster = uint(x.body);
					}
					if (cluster >= 0 && (pos < 0 || track === this.videoId)) pos = cluster;
				}
			}
			if (time >= 0 && pos >= 0) this.cues.push({ time: (time * this.scale) / 1e9, pos: this.segment + pos });
		}
		this.cues.sort((a, b) => a.time - b.time);
		return this.cues;
	}

	async seek(time: number): Promise<number> {
		const first = this.positions.get(ID.Cluster) ?? this.segmentEnd;
		if (time <= 0) {
			this.pos = first;
			return 0;
		}
		const cues = await this.loadCues();
		if (cues.length) {
			let pick = cues[0];
			for (const c of cues) {
				if (c.time <= time) pick = c;
				else break;
			}
			this.pos = pick.pos;
			return pick.time;
		}
		// No cues: bisect over cluster timecodes, landing a few seconds early.
		const target = Math.max(0, time - 3);
		let lo = first;
		let hi = Number.isFinite(this.segmentEnd) ? this.segmentEnd : this.src.size;
		let best = { pos: first, time: 0 };
		for (let i = 0; i < 24 && Number.isFinite(hi) && hi - lo > 1 << 16; i++) {
			const mid = Math.floor((lo + hi) / 2);
			const found = await this.findCluster(mid, Math.min(hi, mid + (4 << 20)));
			if (!found) {
				hi = mid;
				continue;
			}
			if (found.time <= target) {
				best = found;
				lo = found.pos + 1;
			} else hi = mid;
		}
		this.pos = best.pos;
		return best.time;
	}

	/** The first cluster at or after `from`, with its timecode. */
	private async findCluster(from: number, to: number): Promise<{ pos: number; time: number } | null> {
		const step = 1 << 16;
		for (let at = from; at < to; at += step - 4) {
			const b = await this.src.bytes(at, step);
			for (let i = 0; i + 4 < b.length; i++) {
				if (b[i] !== 0x1f || b[i + 1] !== 0x43 || b[i + 2] !== 0xb6 || b[i + 3] !== 0x75) continue;
				const h = header(await this.src.bytes(at + i, 32), 0, at + i);
				if (!h || h.id !== ID.Cluster) continue;
				const first = header(await this.src.bytes(h.data, 16), 0, h.data);
				if (!first || first.id !== ID.Timecode || first.size > 8) continue;
				const tc = uint(await this.src.bytes(first.data, first.size));
				return { pos: at + i, time: (tc * this.scale) / 1e9 };
			}
			if (b.length < step) break;
		}
		return null;
	}

	async read(): Promise<Sample[] | null> {
		for (;;) {
			if (this.pos >= this.segmentEnd) return null;
			const h = header(await this.src.bytes(this.pos, 16), 0, this.pos);
			if (!h) return null;
			if (h.id === ID.Cluster) return this.cluster(h);
			if (h.size === UNKNOWN) return null;
			this.pos = h.data + h.size;
		}
	}

	private async cluster(c: Header): Promise<Sample[]> {
		const end = c.size === UNKNOWN ? this.segmentEnd : c.data + c.size;
		const out: Sample[] = [];
		const laced: Sample[][] = [];
		let base = 0;
		let at = c.data;
		this.src.prefetch(end);
		while (at < end) {
			const h = header(await this.src.bytes(at, 16), 0, at);
			if (!h) {
				at = end;
				break;
			}
			// An open-ended cluster ends where the next top-level element starts.
			if (c.size === UNKNOWN && TOP_LEVEL.has(h.id)) break;
			const size = h.size === UNKNOWN ? 0 : h.size;
			if (h.id === ID.Timecode) base = uint(await this.src.bytes(h.data, size));
			else if (h.id === ID.SimpleBlock) await this.block(await this.src.bytes(h.data, size), base, null, null, out, laced);
			else if (h.id === ID.BlockGroup) {
				const g = await this.src.bytes(h.data, size);
				let block: Uint8Array | null = null;
				let duration: number | null = null;
				let referenced = false;
				for (const x of children(g)) {
					if (x.id === ID.Block) block = x.body;
					else if (x.id === ID.BlockDuration) duration = uint(x.body);
					else if (x.id === ID.ReferenceBlock) referenced = true;
				}
				if (block) await this.block(block, base, duration, !referenced, out, laced);
			}
			at = h.data + size;
		}
		this.pos = at;
		this.spread(out, laced);
		return out;
	}

	/**
	 * Spreads the frames of laced blocks that had no frame time over the gap to the next block of
	 * their track. Frames left on one time stamp play as one, and the sound runs out early.
	 */
	private spread(out: Sample[], laced: Sample[][]) {
		for (const group of laced) {
			const first = group[0];
			const from = out.indexOf(group[group.length - 1]) + 1;
			const next = out.slice(from).find((s) => s.track === first.track && s.pts > first.pts);
			const each = next ? (next.pts - first.pts) / group.length : (this.spreadTimes.get(first.track) ?? 0);
			if (!(each > 0)) continue;
			this.spreadTimes.set(first.track, each);
			group.forEach((s, i) => {
				s.pts = s.dts = first.pts + i * each;
				s.duration = each;
			});
		}
	}

	private async block(b: Uint8Array, base: number, duration: number | null, key: boolean | null, out: Sample[], laced: Sample[][]) {
		// A block starts with the track number as an EBML size-style integer.
		let len = 1;
		while (len <= 8 && !(b[0] & (0x80 >> (len - 1)))) len++;
		let track = b[0] & (0xff >> len);
		for (let i = 1; i < len; i++) track = track * 256 + b[i];
		if (!this.wanted.has(track)) return;
		const rel = ((b[len] << 24) >> 16) | b[len + 1]; // signed 16-bit
		const flags = b[len + 2];
		const frames = unlace(b, len + 3, (flags >> 1) & 3);
		const enc = this.encodings.get(track);
		const t = ((base + rel) * this.scale) / 1e9;
		const d = duration != null ? (duration * this.scale) / 1e9 : 0;
		const isKey = key ?? (flags & 0x80) !== 0;
		// Laced frames share the block's time stamp: each frame starts one frame time after the last.
		// mkvmerge laces audio and writes no BlockDuration, so the frame time comes from the track.
		const each = frames.length > 1 ? (d ? d / frames.length : (this.frameTimes.get(track) ?? 0)) : 0;
		const group: Sample[] = [];
		for (let i = 0; i < frames.length; i++) {
			let data = frames[i];
			if (enc?.strip) {
				const full = new Uint8Array(enc.strip.length + data.length);
				full.set(enc.strip);
				full.set(data, enc.strip.length);
				data = full;
			} else if (enc?.zlib) data = await inflate(data);
			else data = data.slice();
			const pts = t + i * each;
			const s: Sample = { track, pts, dts: pts, duration: frames.length > 1 ? each : d, key: isKey, data };
			out.push(s);
			group.push(s);
		}
		if (frames.length > 1 && !each) laced.push(group);
	}

	close() {
		this.src.close();
	}
}

/**
 * Seconds per frame of an audio codec whose frames always hold the same number of samples, or 0.
 * `rate` is the coded rate (for HE-AAC, half the rate it plays at).
 */
export function codecFrameTime(codec: string, rate: number): number {
	if (!(rate > 0)) return 0;
	const samples =
		codec === 'aac' ? 1024
		: codec === 'ac3' || codec === 'eac3' ? 1536
		: codec === 'mp3' ? (rate < 32000 ? 576 : 1152)
		: codec === 'mp2' ? 1152
		: codec === 'mp1' ? 384
		: codec === 'dts' ? 512
		: 0;
	return samples / rate;
}

/** Old Matroska AAC ids name the profile and carry no AudioSpecificConfig. */
function aacConfigFromId(id: string, t: Track): Uint8Array {
	const rates = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];
	const profile = /MAIN/.test(id) ? 1 : /SSR/.test(id) ? 3 : /LTP/.test(id) ? 4 : 2;
	let rate = t.sampleRate ?? 48000;
	if (/SBR/.test(id)) rate /= 2;
	const idx = Math.max(0, rates.indexOf(rate));
	const ch = t.channels ?? 2;
	return new Uint8Array([(profile << 3) | (idx >> 1), ((idx & 1) << 7) | (ch << 3)]);
}
