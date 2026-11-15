// The audio decoder's worker: loads one WebAssembly decoder family (pcm/<family>.js), decodes the
// packets it is sent, and hands stereo PCM to the AudioWorklet through a MessagePort. It decodes
// only a few seconds ahead of what the worklet plays, so the PCM held stays small.

/// <reference lib="webworker" />

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
	port: MessagePort;
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
	port!.postMessage({ type: 'pcm', gen, pts, rate, channels: m._kd_channels(handle), data: pcm }, [pcm.buffer]);
}

function pump() {
	if (busy || !mod || handle < 0) return;
	busy = true;
	try {
		while (queue.length && decodedTo < playhead + AHEAD) decode(queue.shift()!);
	} finally {
		busy = false;
	}
}

async function open(m: OpenMessage) {
	const url = new URL(`pcm/${m.family}.js`, m.base).href;
	const factory = (await import(/* @vite-ignore */ url)).default as (o: object) => Promise<DecoderModule>;
	mod = await factory({ locateFile: (f: string) => new URL(`pcm/${f}`, m.base).href });
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
	port = m.port;
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
		case 'packets':
			if (m.gen !== gen) break;
			for (const p of m.packets as Packet[]) queue.push(p);
			pump();
			break;
		case 'flush':
			gen = m.gen;
			queue = [];
			playhead = m.time;
			decodedTo = -Infinity;
			clock = null;
			if (mod && handle >= 0) mod._kd_flush(handle);
			break;
		case 'stats':
			scope.postMessage({ type: 'stats', cpu, decoded: decodedSeconds, queued: queue.length, heap: mod?.HEAPU8.length ?? 0 });
			break;
		case 'close':
			if (mod && handle >= 0) mod._kd_close(handle);
			handle = -1;
			mod = null;
			scope.close();
			break;
	}
};
