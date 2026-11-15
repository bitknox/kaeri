// Typeset (ASS) subtitles, drawn by JASSUB (libass in WebAssembly) on a canvas over the video,
// with the file's own fonts from the sender's relay. JASSUB and its WebAssembly are separate files
// under vendor/jassub/, loaded only when an episode has typeset subtitles.

import { FrameClock } from './frame-clock';
import type { TypesetSink } from './formats/subs';

export type Failure = 'subtitle' | 'font' | 'renderer';

interface Jassub {
	debug: unknown;
	renderer?: { processChunk(data: string, timecode: number, duration: number): Promise<void> | void };
	destroy(): Promise<void>;
	ready: Promise<void>;
}
type JassubClass = new (opts: Record<string, unknown>) => Jassub;

let loaded: Promise<JassubClass> | null = null;

/** Whether this TV's browser has what JASSUB needs. */
export function canTypeset(): boolean {
	return (
		typeof WebAssembly === 'object' &&
		typeof Worker === 'function' &&
		typeof OffscreenCanvas === 'function' &&
		typeof HTMLVideoElement !== 'undefined' &&
		'requestVideoFrameCallback' in HTMLVideoElement.prototype &&
		'transferControlToOffscreen' in HTMLCanvasElement.prototype
	);
}

function vendor(base: string) {
	const at = (p: string) => new URL(`vendor/jassub/${p}`, base).href;
	return {
		script: at('jassub.js'),
		workerUrl: at('worker.js'),
		wasmUrl: at('jassub-worker.wasm'),
		modernWasmUrl: at('jassub-worker-modern.wasm'),
		defaultFont: at('default.woff2'),
	};
}

async function bytes(url: string): Promise<Uint8Array> {
	const r = await fetch(url);
	if (!r.ok) throw new Error(`${r.status}`);
	return new Uint8Array(await r.arrayBuffer());
}

export class Typeset implements TypesetSink {
	private jassub: Jassub | null = null;
	private clock: FrameClock | null = null;
	private token = 0;

	constructor(
		private readonly video: HTMLVideoElement,
		private readonly base: string,
	) {}

	get active() {
		return this.jassub != null;
	}

	/**
	 * Draws `ass` with `fonts`. Resolves once the renderer runs; rejects with the part that failed,
	 * and the caller shows the WebVTT twin instead.
	 */
	async show(ass: string, fonts: string[]): Promise<void> {
		await this.hide();
		const token = ++this.token;
		let content: string;
		try {
			const r = await fetch(ass);
			if (!r.ok) throw new Error(`${r.status}`);
			content = await r.text();
		} catch {
			throw 'subtitle' as Failure;
		}
		let fontData: Uint8Array[];
		try {
			fontData = await Promise.all(fonts.map(bytes));
		} catch {
			throw 'font' as Failure;
		}
		if (token !== this.token) return;
		await this.run(token, content, fontData);
	}

	/** Starts the renderer with an ASS track's header and the file's fonts; events come by `chunk`. */
	async start(header: string, fonts: Uint8Array[]): Promise<void> {
		await this.hide();
		await this.run(++this.token, header, fonts);
	}

	/** One event of the track: "ReadOrder, Layer, Style, ..., Text", times in seconds. */
	chunk(text: string, start: number, duration: number) {
		void Promise.resolve(this.jassub?.renderer?.processChunk(text, Math.round(start * 1000), Math.round(duration * 1000))).catch(() => {});
	}

	stop() {
		void this.hide();
	}

	private async run(token: number, content: string, fontData: Uint8Array[]) {
		const v = vendor(this.base);
		try {
			loaded ??= import(/* @vite-ignore */ v.script).then((m) => m.default as JassubClass);
			const JASSUB = await loaded;
			if (token !== this.token) return;
			const jassub = new JASSUB({
				video: this.video,
				subContent: content,
				fonts: fontData,
				workerUrl: v.workerUrl,
				wasmUrl: v.wasmUrl,
				modernWasmUrl: v.modernWasmUrl,
				availableFonts: { 'liberation sans': v.defaultFont },
				queryFonts: false,
				// A 1080p TV draws at most 1080 lines; more costs time and shows nothing.
				prescaleHeightLimit: 1080,
				maxRenderHeight: 1080,
			});
			const clock = new FrameClock();
			jassub.debug = clock;
			this.jassub = jassub;
			this.clock = clock;
			await jassub.ready;
		} catch (e) {
			console.warn('typeset subtitles', e);
			await this.hide();
			throw 'renderer' as Failure;
		}
	}

	/** Once a second: true when rendering has fallen behind for five seconds. */
	tick(playing: boolean): boolean {
		return this.clock?.tick(playing) ?? false;
	}

	async hide() {
		this.token++;
		const j = this.jassub;
		this.jassub = null;
		this.clock = null;
		if (j) await j.destroy().catch(() => {});
	}
}
