// Kaeri's messages between the sender (the engine) and this receiver, on one Cast namespace, and
// Kaeri's part of a LOAD. The engine owns both; crates/kaeri-ffi/src/sender.rs is the other side.

export const NAMESPACE = 'urn:x-cast:app.kaeri.cast';
export const VERSION = 1;

/** The device that casts, which the receiver's lines name ("Waiting for the iPhone"). */
export type Device = 'iphone' | 'ipad' | 'mac' | 'phone' | 'tablet' | 'computer';
export const DEVICES: readonly Device[] = ['iphone', 'ipad', 'mac', 'phone', 'tablet', 'computer'];

export function asDevice(value: unknown): Device | null {
	return typeof value === 'string' && (DEVICES as readonly string[]).includes(value)
		? (value as Device)
		: null;
}

/** A guess from the sender's user agent, until its `hello` names the device. */
export function deviceFromAgent(agent: string | undefined): Device {
	const a = agent ?? '';
	if (/iPad/.test(a)) return 'ipad';
	if (/iPhone|iOS/.test(a)) return 'iphone';
	if (/Mac ?OS|Macintosh/.test(a)) return 'mac';
	if (/Windows|Linux(?!.*Android)|X11/.test(a)) return 'computer';
	return 'phone';
}

/** A side-loaded subtitle file, served by the relay. */
export interface SubtitleFile {
	/** The WebVTT track's `trackId` in `media.tracks`. */
	id: number;
	url: string;
	format: 'ass' | 'ssa' | 'srt' | 'vtt' | 'pgs' | 'vobsub' | string;
	language?: string;
	title?: string;
	/** Fonts for this file, when they differ from the load's `fonts`. */
	fonts?: string[];
}

/**
 * Kaeri's part of a LOAD, in `media.customData.kaeri` (or the request's `customData.kaeri`).
 * Every field is optional; a LOAD without it plays like the standard receiver would.
 */
export interface LoadExtras {
	version?: number;
	url?: string;
	route?: 'direct' | 'relay';
	container?: string;
	segments?: string | null;
	/** The engine's codec words: `h264`, `hevc`, `hevc-main10`, `vp9`, `av1` or `video:<name>`. */
	video?: { codec?: string; height?: number | null } | null;
	/** `aac`, `opus`, `flac`, `ac3`, `eac3`, `dts`, `truehd` or `audio:<name>`. */
	audio?: { index?: number; language?: string; codec?: string } | null;
	subtitles?: SubtitleFile[];
	/** The subtitle file that is on, by `id`, or null. */
	subtitle?: number | null;
	/** The relay's URLs for the fonts typeset subtitles use (the file's attachments). */
	fonts?: string[];
	/** The episode's number, when the metadata has none. */
	episode?: string | number;
	/** Wide art for Starting, Couldn't play and Finished, when the metadata's image is a poster. */
	still?: string;
	/** The sender knows this TV draws plain subtitles only. */
	plainSubtitles?: boolean;
	/** The sender's language and device, so the first screen is right before `hello`. */
	language?: string;
	device?: string;
}

export function readExtras(request: unknown): LoadExtras {
	const r = request as { customData?: { kaeri?: unknown }; media?: { customData?: { kaeri?: unknown } } } | null;
	const raw = r?.media?.customData?.kaeri ?? r?.customData?.kaeri;
	return raw && typeof raw === 'object' ? (raw as LoadExtras) : {};
}

/** The typeset (ASS or SSA) files JASSUB draws, by the WebVTT track they stand in for. */
export function typesetFiles(extras: LoadExtras): { trackId: number; ass: string; fonts: string[] }[] {
	return (extras.subtitles ?? [])
		.filter((s) => typeof s?.url === 'string' && typeof s.id === 'number' && /^(ass|ssa)$/i.test(String(s.format)))
		.map((s) => ({ trackId: s.id, ass: s.url, fonts: s.fonts ?? extras.fonts ?? [] }));
}

export function message(type: string, body: Record<string, unknown> = {}) {
	return { type, version: VERSION, ...body };
}
