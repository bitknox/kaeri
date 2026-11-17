// The testing build's status line: whether sound is decoded (and, for WebAssembly sound, how
// loud it is before the volume), and whether video frames are decoded and shown. Muted TV runs
// read it to prove sound: on screen, in `__kaeriDebug()`, once a second in the console, and on
// the dev host in the dev server's log. It runs on the dev receiver's host (cast-dev.*) or with
// `?debug` in the page's URL, never on kaeri.app.

import type { Formats } from './formats/receiver';

export function debugOn(loc: Location = location): boolean {
	return /^cast-dev\./.test(loc.hostname) || new URLSearchParams(loc.search).has('debug');
}

/** Sends a line to the dev server's /log, on the dev receiver's host only. */
export function devLog(line: string, loc: Location = location) {
	if (!/^cast-dev\./.test(loc.hostname)) return;
	try {
		void fetch(new URL('log', loc.href).href, { method: 'POST', body: line.slice(0, 2000), keepalive: true }).catch(() => {});
	} catch {
		// the dev server is gone
	}
}

/**
 * On the dev receiver's host, page errors go to the dev server's /log, so a run without DevTools
 * still reads them. The console itself stays as it is: JASSUB's worker calls into it.
 */
export function forwardErrors(loc: Location = location) {
	if (!/^cast-dev\./.test(loc.hostname)) return;
	addEventListener('error', (e) => devLog(`page error ${e.message} at ${e.filename}:${e.lineno}`, loc));
	addEventListener('unhandledrejection', (e) => devLog(`unhandled ${String((e.reason as Error)?.message ?? e.reason)}`, loc));
}

export interface DebugStatus {
	/** Who decodes the sound: the receiver's WebAssembly, MediaSource, or CAF's player. */
	audioPath: 'wasm' | 'mse' | 'caf' | 'none';
	/** WebAssembly: seconds of sound decoded. MSE and CAF: bytes the element decoded. */
	audioDecoded: number | null;
	audioUnit: 's' | 'bytes' | null;
	/**
	 * Loudest level over the last second before the volume (dBFS, -120 for silence), for
	 * WebAssembly sound; null for MSE and CAF sound, whose decoded bytes are the proof.
	 */
	level: number | null;
	/** Video frames decoded (less dropped) and shown since the load, where the TV counts them. */
	videoDecoded: number | null;
	videoShown: number | null;
	time: number;
}

type CountingVideo = HTMLVideoElement & { webkitAudioDecodedByteCount?: number };

export class DebugStatusLine {
	status: DebugStatus | null = null;
	private el: HTMLElement;
	private loudest = 0;
	private buf = new Float32Array(2048);
	private decodedSeconds: number | null = null;
	private shownBase = 0;
	private lastSetup = '';

	constructor(
		private readonly video: HTMLVideoElement,
		private readonly formats: Formats,
		private readonly shown: () => number | null,
	) {
		this.el = document.createElement('div');
		this.el.id = 'debug-status';
		this.el.setAttribute('aria-hidden', 'true');
		Object.assign(this.el.style, {
			position: 'fixed',
			left: '24px',
			bottom: '24px',
			zIndex: '99',
			padding: '6px 10px',
			font: '16px/1.3 ui-monospace, monospace',
			color: '#fff',
			background: 'rgba(0, 0, 0, 0.6)',
			borderRadius: '6px',
			pointerEvents: 'none',
			whiteSpace: 'pre',
		});
		document.body.appendChild(this.el);
		video.addEventListener('loadstart', () => {
			this.shownBase = this.shown() ?? 0;
			this.decodedSeconds = null;
		});
		setInterval(() => this.meter(), 100);
		setInterval(() => this.report(), 1000);
	}

	/**
	 * The analyser that hears the sound: the WebAssembly output only. Capturing the element's own
	 * sound (captureStream) makes JASSUB fail to start on the TV, so MSE and CAF sound shows its
	 * decoded bytes and no level.
	 */
	private analyser(): AnalyserNode | null {
		return this.formats.current?.pcmAnalyser() ?? null;
	}

	private meter() {
		const a = this.analyser();
		if (!a) return;
		if (this.buf.length !== a.fftSize) this.buf = new Float32Array(a.fftSize);
		a.getFloatTimeDomainData(this.buf);
		let sum = 0;
		for (const x of this.buf) sum += x * x;
		this.loudest = Math.max(this.loudest, Math.sqrt(sum / this.buf.length));
	}

	private report() {
		const u = this.formats.current;
		const stats = u?.stats();
		const path: DebugStatus['audioPath'] = stats ? (stats.audioPath ?? 'none') : this.video.currentSrc ? 'caf' : 'none';
		if (path === 'wasm') void u?.pcmCpu()?.then((c) => (this.decodedSeconds = c.decoded));
		const v = this.video as CountingVideo;
		const quality = typeof v.getVideoPlaybackQuality === 'function' ? v.getVideoPlaybackQuality() : null;
		const heard = this.analyser() != null;
		this.status = {
			audioPath: path,
			audioDecoded: path === 'wasm' ? this.decodedSeconds : (v.webkitAudioDecodedByteCount ?? null),
			audioUnit: path === 'wasm' ? 's' : v.webkitAudioDecodedByteCount != null ? 'bytes' : null,
			level: heard ? Math.max(-120, Math.round(20 * Math.log10(this.loudest) * 10) / 10) : null,
			videoDecoded: quality ? quality.totalVideoFrames - quality.droppedVideoFrames : null,
			videoShown: this.shown() == null ? null : this.shown()! - this.shownBase,
			time: Math.round(v.currentTime * 10) / 10,
		};
		this.loudest = 0;
		const s = this.status;
		const decoded = s.audioDecoded == null ? '?' : s.audioUnit === 's' ? `${s.audioDecoded.toFixed(1)} s` : `${s.audioDecoded} bytes`;
		const level = s.level == null ? '?' : s.level <= -120 ? 'silent' : `${s.level.toFixed(1)} dBFS`;
		const p = stats?.pcm;
		const pcm = p
			? `\npcm queued ${p.queued.toFixed(2)} s drift ${ms(p.drift)} worst ${ms(p.worst)} jumps ${p.jumps} waits ${p.waits ?? 0} starved ${p.starved} jitter ${ms(p.jitter ?? 0)}${p.clock ? ` clock video spread ${ms(p.clock.video)} high ${ms(p.clock.videoHigh)} ctx spread ${ms(p.clock.ctx)} ts spread ${ms(p.clock.ts ?? 0)} latency ${ms(p.clock.latency ?? 0)}` : ''}${stats?.audioGate ? ` gate ${stats.audioGate}` : ''}`
			: '';
		const line = `audio ${s.audioPath} ${stats?.audio ?? ''} decoded ${decoded} level ${level}\nvideo ${stats?.video ?? ''} decoded ${s.videoDecoded ?? '?'} shown ${s.videoShown ?? '?'} at ${s.time} s${pcm}`;
		const why = stats?.audioWhy ? `\nwhy ${stats.audioWhy}` : '';
		this.el.textContent = line + why;
		console.info('[debug]', line.replaceAll('\n', ' | '));
		devLog(`status ${line.replaceAll('\n', ' | ')}`);
		// The audio's setup, and why it plays through WebAssembly, once per change.
		const setup = stats ? `setup ${stats.format} audio ${stats.audioPath ?? 'none'} ${stats.audioSetup ?? 'no audio track'}${stats.audioWhy ? ` | why ${stats.audioWhy}` : ''}` : '';
		if (setup && setup !== this.lastSetup) {
			this.lastSetup = setup;
			console.info('[debug]', setup);
			devLog(setup);
		}
	}
}

/** Seconds as signed milliseconds. */
function ms(seconds: number): string {
	const v = Math.round(seconds * 1000);
	return `${v > 0 ? '+' : ''}${v} ms`;
}
