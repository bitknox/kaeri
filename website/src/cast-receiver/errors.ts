// Turns the player's error into what the sender needs to change only the failing part: the
// stream's video, its audio, the subtitles, the network (the relay or the source stopped
// answering) or the playlist. Nothing here guesses: a media error blames video or audio only when
// a codec or the failing buffer says which.

export type Part = 'video' | 'audio' | 'subtitles' | 'network' | 'manifest' | 'unknown';

/** What was being fetched when it failed. */
export type Fetched = 'playlist' | 'init' | 'segment' | 'subtitle' | 'font' | null;

export interface Report {
	part: Part;
	/** The codec string the TV refused, when the error or a check names one. */
	codec: string | null;
	/** A short technical line for Messages: the player's own names and codes. */
	detail: string;
	fetched: Fetched;
	/** HTTP status; 0 when nothing answered. Null when it wasn't a fetch. */
	status: number | null;
	/** CAF's detailed error code. */
	detailed: number | null;
	/** Shaka's error code. */
	shaka: number | null;
	/** Nothing answered: the relay or the source is gone, which may come back. */
	lost: boolean;
}

const AUDIO = /^(mp4a|opus|flac|ac-3|ec-3|mp3|\.mp3|alac|dts|vorbis|mlpa|aac|ac3|eac3|truehd|audio:)/i;
const VIDEO = /^(avc[13]|hvc1|hev1|dvh1|dvhe|vp0?[89]|vp09|av01|h264|hevc|av1|video:)/i;

/** A codec string the TV's MediaSource can be asked about, for each of the engine's codec words. */
const PROBES: Record<string, string> = {
	h264: 'video/mp4; codecs="avc1.640028"',
	hevc: 'video/mp4; codecs="hvc1.1.6.L120.90"',
	'hevc-main10': 'video/mp4; codecs="hvc1.2.4.L120.90"',
	vp9: 'video/webm; codecs="vp09.00.40.08"',
	av1: 'video/mp4; codecs="av01.0.08M.08"',
	aac: 'audio/mp4; codecs="mp4a.40.2"',
	opus: 'audio/webm; codecs="opus"',
	flac: 'audio/mp4; codecs="flac"',
	ac3: 'audio/mp4; codecs="ac-3"',
	eac3: 'audio/mp4; codecs="ec-3"',
	dts: 'audio/mp4; codecs="dtsc"',
	truehd: 'audio/mp4; codecs="mlpa"',
};

/**
 * After a failure only: which of the load's codecs (the engine's words, video first) this TV
 * says no to. A word with no probe is never blamed.
 */
export function refusedOf(codecs: (string | null | undefined)[], supported: (type: string) => boolean): string[] {
	return codecs.filter((c): c is string => {
		const probe = c ? PROBES[c.toLowerCase()] : undefined;
		if (!probe) return false;
		try {
			return !supported(probe);
		} catch {
			return false;
		}
	});
}
const CODEC_IN_TEXT = /\b(avc[13]\.[0-9a-f]{6}|hvc1\.[\w.]+|hev1\.[\w.]+|vp09\.[\d.]+|av01\.[\w.]+|mp4a\.[\da-f.]+|opus|flac|ac-3|ec-3)\b/i;

export function codecPart(codec: string): 'video' | 'audio' | null {
	if (AUDIO.test(codec)) return 'audio';
	if (VIDEO.test(codec)) return 'video';
	return null;
}

/** Shaka's request types, from its NetworkingEngine. */
const REQUEST_MANIFEST = 0;
const REQUEST_SEGMENT = 1;
const REQUEST_KEY = 6;

interface Raw {
	detailedErrorCode?: number;
	error?: unknown;
	reason?: string;
}

function flat(value: unknown, depth = 0, out: unknown[] = []): unknown[] {
	if (depth > 4 || value == null) return out;
	out.push(value);
	if (Array.isArray(value)) value.forEach((v) => flat(v, depth + 1, out));
	else if (typeof value === 'object')
		for (const v of Object.values(value as Record<string, unknown>)) flat(v, depth + 1, out);
	return out;
}

function shakaOf(error: unknown): { code: number | null; data: unknown[] } {
	const e = error as Record<string, unknown> | null;
	if (!e || typeof e !== 'object') return { code: null, data: [] };
	const code = [e.shakaErrorCode, e.code, (e.shakaError as Record<string, unknown>)?.code].find(
		(c) => typeof c === 'number' && c >= 1000 && c < 10000,
	) as number | undefined;
	const data = [e.shakaErrorData, e.data, (e.shakaError as Record<string, unknown>)?.data].find(Array.isArray) as
		| unknown[]
		| undefined;
	return { code: code ?? null, data: data ?? [] };
}

/**
 * The report for one player error. `names` maps CAF's detailed codes to their names
 * (`cast.framework.events.DetailedErrorCode` reversed); `refused` are the load's codecs the
 * TV's MediaSource says no to, asked after the failure (`refusedOf`).
 */
export function classify(raw: Raw, names: Record<number, string>, refused: string[] = []): Report {
	const detailed = typeof raw.detailedErrorCode === 'number' ? raw.detailedErrorCode : null;
	const name = detailed != null ? (names[detailed] ?? String(detailed)) : '';
	const { code: shaka, data } = shakaOf(raw.error);
	const everything = flat(raw.error);
	const texts = everything.filter((v): v is string => typeof v === 'string');
	const codec =
		texts.map((t) => t.match(CODEC_IN_TEXT)?.[1]).find(Boolean) ?? refused[0] ?? null;

	let part: Part = 'unknown';
	let fetched: Fetched = null;
	let status: number | null = null;
	let lost = false;

	if (shaka != null && shaka >= 1000 && shaka < 2000) {
		// Network: BAD_HTTP_STATUS [uri, status, text, headers, type], HTTP_ERROR [uri, error, type],
		// TIMEOUT [uri, type].
		const type = [...data].reverse().find((v) => typeof v === 'number' && v < 20) as number | undefined;
		status = shaka === 1001 && typeof data[1] === 'number' ? data[1] : 0;
		fetched = type === REQUEST_MANIFEST ? 'playlist' : type === REQUEST_KEY ? 'init' : type === REQUEST_SEGMENT ? 'segment' : 'segment';
		part = fetched === 'playlist' ? 'manifest' : 'network';
		lost = status === 0;
	} else if (shaka != null && shaka >= 2000 && shaka < 3000) {
		part = 'subtitles';
		fetched = 'subtitle';
	} else if (shaka != null && shaka >= 4000 && shaka < 5000 && shaka !== 4032) {
		part = 'manifest';
		fetched = 'playlist';
	} else if (/MANIFEST|PLAYLIST/.test(name) && /NETWORK/.test(name)) {
		part = 'manifest';
		fetched = 'playlist';
		status = statusIn(everything);
		lost = status === 0;
	} else if (/NETWORK|SEGMENT|NO_INIT|KEY_LOAD/.test(name)) {
		part = 'network';
		fetched = /INIT|KEY/.test(name) ? 'init' : 'segment';
		status = statusIn(everything);
		lost = status === 0;
	} else if (/MANIFEST|PLAYLIST|PARSING/.test(name)) {
		part = 'manifest';
		fetched = 'playlist';
	} else if (/TEXT|CAPTION/.test(name)) {
		part = 'subtitles';
		fetched = 'subtitle';
	}

	if (part === 'unknown') {
		// A media error: blame the codec that was named or refused, else the buffer that failed.
		const byCodec = codec ? codecPart(codec) : null;
		const buffer = texts.find((t) => t === 'audio' || t === 'video') as 'audio' | 'video' | undefined;
		part = byCodec ?? buffer ?? 'unknown';
	}

	const message = texts.find((t) => t.length > 3 && t.length < 160 && !/^https?:/.test(t));
	const detail = [name, shaka != null ? `shaka ${shaka}` : '', raw.reason ?? '', message ?? '']
		.filter(Boolean)
		.join(' · ');
	return { part, codec: codec ?? null, detail, fetched, status, detailed, shaka, lost };
}

function statusIn(values: unknown[]): number {
	for (const v of values) {
		if (v && typeof v === 'object') {
			const o = v as Record<string, unknown>;
			for (const key of ['status', 'httpStatus', 'statusCode'])
				if (typeof o[key] === 'number') return o[key] as number;
		}
	}
	return 0;
}
