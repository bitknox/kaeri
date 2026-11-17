// The audio decoder's worker: loads one WebAssembly decoder family (pcm/<family>.js), decodes the
// packets it is sent, and hands stereo PCM to the AudioWorklet through a MessagePort. It decodes
// only a few seconds ahead of what the worklet plays, so the PCM held stays small.

/// <reference lib="webworker" />

import { MAX_BLOCK, encodeFrame } from './flac-encode';

interface DecoderModule {
	HEAPU8: Uint8Array;
	HEAPF32: Float32Array;
	_kd_open(name: number, extra: number, extraLen: number, rate: number, channels: number, parse: number): number;
	_kd_decode(h: number, data: number, len: number): number;
	_kd_out(h: number): number;
	_kd_rate(h: number): number;
	_kd_channels(h: number): number;
	_kd_flush(h: number): void;
	_kd_close(h: number): void;
	_malloc(n: number): number;
	_free(p: number): void;
	stringToNewUTF8(s: string): number;
}

export interface OpenMessage {
	type: 'open';
	base: string;
	family: string;
	decoder: string;
	config?: Uint8Array;
	rate: number;
	channels: number;
	unframed: boolean;
	/** Where the sound goes: PCM to the worklet through `port`, or FLAC frames back to the main thread. */
	out?: 'worklet' | 'flac';
	port?: MessagePort;
}

interface Packet {
	pts: number;
	data: Uint8Array;
}

/** Seconds of PCM kept decoded ahead of the worklet's position. */
const AHEAD = 4;

let mod: DecoderModule | null = null;
let handle = -1;
let unframed = false;
let port: MessagePort | null = null;
let gen = 0;
let queue: Packet[] = [];
/** The worklet's position, media seconds. */
let playhead = 0;
/** End of the PCM sent, media seconds. */
let decodedTo = -Infinity;
/** Running clock for parsers whose output frames don't line up with the packets. */
let clock: number | null = null;
let inBuf = 0;
let inCap = 0;
let busy = false;
let flac = false;
/** Sample number of the next FLAC frame (the frame headers count samples). */
let flacSamples = 0;
/** ms spent encoding FLAC. */
let encodeCpu = 0;
let cpu = 0;
let decodedSeconds = 0;

const scope = self as unknown as DedicatedWorkerGlobalScope;

function input(data: Uint8Array): number {
	if (data.length > inCap) {
		if (inBuf) mod!._free(inBuf);
		inCap = Math.max(data.length, 1 << 16);
		inBuf = mod!._malloc(inCap);
	}
	mod!.HEAPU8.set(data, inBuf);
	return inBuf;
}

function decode(p: Packet) {
	const m = mod!;
	const t0 = performance.now();
	const n = m._kd_decode(handle, input(p.data), p.data.length);
	if (n < 0) {
		scope.postMessage({ type: 'error', detail: 'decode failed' });
		return;
	}
	if (n === 0) return;
	const rate = m._kd_rate(handle) || 48000;
	const at = m._kd_out(handle) >> 2;
	const pcm = m.HEAPF32.slice(at, at + n * 2);
	let pts = p.pts;
	if (unframed) {
		// The parser may hold bytes back: follow its output with a running clock, re-anchored when
		// a packet's time disagrees by more than 60 ms.
		if (clock == null || Math.abs(clock - p.pts) > 0.06 + n / rate) clock = p.pts;
		pts = clock;
		clock += n / rate;
	}
	cpu += performance.now() - t0;
	decodedSeconds += n / rate;
	decodedTo = pts + n / rate;
	if (flac) toFlac(pcm, n, rate, pts);
	else port!.postMessage({ type: 'pcm', gen, pts, rate, channels: m._kd_channels(handle), data: pcm }, [pcm.buffer]);
}

type FlacOut = { pts: number; samples: number; data: Uint8Array };
/** FLAC frames encoded in this pump, sent together at its end (one append on the main thread). */
let flacOut: { rate: number; frames: FlacOut[] } | null = null;

/** Encodes decoded stereo PCM as FLAC frames of at most MAX_BLOCK samples for the main thread. */
function toFlac(pcm: Float32Array, n: number, rate: number, pts: number) {
	const t0 = performance.now();
	if (flacOut && flacOut.rate !== rate) sendFlac();
	flacOut ??= { rate, frames: [] };
	const frames = flacOut.frames;
	for (let at = 0; at < n; at += MAX_BLOCK) {
		const count = Math.min(MAX_BLOCK, n - at);
		frames.push({ pts: pts + at / rate, samples: count, data: encodeFrame(pcm.subarray(at * 2, (at + count) * 2), count, 2, flacSamples) });
		flacSamples += count;
	}
	encodeCpu += performance.now() - t0;
}

function sendFlac() {
	const out = flacOut;
	flacOut = null;
	if (out?.frames.length) scope.postMessage({ type: 'flac', gen, rate: out.rate, frames: out.frames }, out.frames.map((f) => f.data.buffer as ArrayBuffer));
}

function pump() {
	if (busy || !mod || handle < 0) return;
	busy = true;
	try {
		// Into MediaSource, everything the engine sends is decoded at once: it paces its reading.
		while (queue.length && (flac || decodedTo < playhead + AHEAD)) decode(queue.shift()!);
	} finally {
		busy = false;
		sendFlac();
	}
}

type Factory = (o: object) => Promise<DecoderModule>;

/** Decoder families fetched and compiled ahead of use: the receiver starts this worker before a load needs it. */
const prepared = new Map<string, Promise<{ factory: Factory; wasm: WebAssembly.Module }>>();

function prepare(base: string, family: string) {
	let p = prepared.get(family);
	if (!p) {
		p = Promise.all([
			import(/* @vite-ignore */ new URL(`pcm/${family}.js`, base).href).then((x) => x.default as Factory),
			compile(new URL(`pcm/${family}.wasm`, base).href),
		]).then(([factory, wasm]) => ({ factory, wasm }));
		// A failure shows when the family is opened.
		p.catch(() => {});
		prepared.set(family, p);
	}
	return p;
}

async function compile(url: string): Promise<WebAssembly.Module> {
	try {
		return await WebAssembly.compileStreaming(fetch(url));
	} catch {
		// Served without the application/wasm type.
		const r = await fetch(url);
		if (!r.ok) throw new Error(`${url}: ${r.status}`);
		return WebAssembly.compile(await r.arrayBuffer());
	}
}

async function open(m: OpenMessage) {
	const { factory, wasm } = await prepare(m.base, m.family);
	mod = await new Promise<DecoderModule>((resolve, reject) => {
		factory({
			locateFile: (f: string) => new URL(`pcm/${f}`, m.base).href,
			instantiateWasm: (imports: WebAssembly.Imports, done: (i: WebAssembly.Instance, w: WebAssembly.Module) => void) => {
				WebAssembly.instantiate(wasm, imports).then((i) => done(i, wasm), reject);
				return {};
			},
		}).then(resolve, reject);
	});
	const name = mod.stringToNewUTF8(m.decoder);
	let extra = 0;
	let extraLen = 0;
	if (m.config?.length) {
		extraLen = m.config.length;
		extra = mod._malloc(extraLen);
		mod.HEAPU8.set(m.config, extra);
	}
	handle = mod._kd_open(name, extra, extraLen, Math.round(m.rate || 0), m.channels || 0, m.unframed ? 1 : 0);
	mod._free(name);
	if (extra) mod._free(extra);
	if (handle < 0) throw new Error(`decoder ${m.decoder} didn't open (${handle})`);
	unframed = m.unframed;
	flac = m.out === 'flac';
	if (flac) return;
	port = m.port!;
	port.onmessage = (e) => {
		if (e.data?.type === 'position' && e.data.gen === gen) {
			playhead = e.data.time;
			pump();
		}
	};
}

scope.onmessage = async (e: MessageEvent) => {
	const m = e.data;
	switch (m.type) {
		case 'open':
			try {
				await open(m as OpenMessage);
				scope.postMessage({ type: 'open' });
			} catch (err) {
				scope.postMessage({ type: 'error', detail: String((err as Error)?.message ?? err) });
			}
			break;
		case 'prepare':
			void prepare(m.base, m.family);
			break;
		case 'packets':
			if (m.gen !== gen) break;
			for (const p of m.packets as Packet[]) queue.push(p);
			pump();
			break;
		case 'flush':
			gen = m.gen;
			queue = [];
			flacOut = null;
			playhead = m.time;
			decodedTo = -Infinity;
			clock = null;
			if (mod && handle >= 0) mod._kd_flush(handle);
			break;
		case 'drain':
			// The end of the stream: everything sent is decoded by now.
			pump();
			scope.postMessage({ type: 'drained', gen: m.gen });
			break;
		case 'stats':
			scope.postMessage({ type: 'stats', cpu, encode: encodeCpu, decoded: decodedSeconds, queued: queue.length, heap: mod?.HEAPU8.length ?? 0 });
			break;
		case 'close':
			if (mod && handle >= 0) mod._kd_close(handle);
			handle = -1;
			mod = null;
			scope.close();
			break;
	}
};
