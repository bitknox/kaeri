// Kaeri's messages between the sender (the engine) and this receiver, on one Cast namespace. The
// engine owns the protocol: ~/anistream-notes/design/cast-overhaul/engine-api.md, and
// docs/cast-receiver.md in the repository for what this page does with each message.

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

/**
 * Kaeri's part of a LOAD, in `media.customData.kaeri` (or the request's `customData.kaeri`).
 * Every field is optional; a LOAD without it plays like the standard receiver would.
 */
export interface LoadExtras {
	/** The episode's number, for "Couldn't play episode 7" and "Finished episode 7". */
	episode?: string | number;
	/** Wide art for Starting, Couldn't play and Finished, when the metadata's image is a poster. */
	still?: string;
	/** Typeset twins of the WebVTT tracks, by the text track's id. */
	subtitles?: { trackId: number; ass: string; fonts?: string[] }[];
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

export function message(type: string, body: Record<string, unknown> = {}) {
	return { type, version: VERSION, ...body };
}
