// Reads a file over HTTP in ranges, for the unpackers that need random access (Matroska, MP4,
// MPEG-TS). Bytes are fetched in aligned chunks and a few recent chunks are kept, which covers the
// sequential reads of playback and the short jumps of seeking without holding the file.

/** The stream is one this receiver's player leaves to CAF's own (live, or with keys). */
export class LeaveToCaf extends Error {
	constructor(readonly reason: 'live' | 'keys') {
		super(`left to CAF: ${reason}`);
	}
}

/** A fetch that failed, with what the player's error report needs. */
export class FetchError extends Error {
	constructor(
		readonly url: string,
		/** HTTP status; 0 when nothing answered. */
		readonly status: number,
		readonly what: 'playlist' | 'init' | 'segment' | 'subtitle',
	) {
		super(`${what} ${status || 'no answer'}`);
	}
}

export interface FetchOptions {
	signal?: AbortSignal;
	what?: FetchError['what'];
	range?: [number, number];
	/** Tries again this many times on no answer or a 5xx; 0 to fail at once. */
	retries?: number;
	/** The first wait before trying again, in ms; each wait doubles up to four seconds. */
	backoff?: number;
	/** Ms to wait for an answer before the try counts as no answer. */
	timeout?: number;
}

/** A wait that ends early, with the signal's reason, when the signal aborts. */
function wait(ms: number, signal?: AbortSignal) {
	return new Promise<void>((resolve, reject) => {
		if (signal?.aborted) return reject(signal.reason);
		const done = () => {
			clearTimeout(timer);
			reject(signal!.reason);
		};
		const timer = setTimeout(() => {
			signal?.removeEventListener('abort', done);
			resolve();
		}, ms);
		signal?.addEventListener('abort', done, { once: true });
	});
}

/** fetch, with the receiver's error and quiet retries for a relay that hiccuped. */
export async function get(url: string, o: FetchOptions = {}): Promise<Response> {
	const headers: Record<string, string> = {};
	if (o.range) headers.Range = `bytes=${o.range[0]}-${o.range[1]}`;
	const tries = o.retries ?? 2;
	const backoff = o.backoff ?? 500;
	for (let attempt = 0; ; attempt++) {
		let status = 0;
		try {
			const r = await answer(url, headers, o);
			if (r.ok) return r;
			status = r.status;
			if (status < 500 && status !== 429) throw new FetchError(url, status, o.what ?? 'segment');
		} catch (e) {
			if (e instanceof FetchError) throw e;
			if (o.signal?.aborted) throw e;
		}
		if (attempt >= tries) throw new FetchError(url, status, o.what ?? 'segment');
		await wait(Math.min(backoff * 2 ** attempt, 4000), o.signal);
	}
}

/** One fetch, given up when no answer comes within the timeout. */
async function answer(url: string, headers: Record<string, string>, o: FetchOptions): Promise<Response> {
	if (!o.timeout) return fetch(url, { headers, signal: o.signal });
	// The body still reads after the answer, so the outer signal stays wired to it.
	const ac = new AbortController();
	const outer = o.signal;
	const pass = () => ac.abort(outer?.reason);
	if (outer?.aborted) pass();
	else outer?.addEventListener('abort', pass, { once: true });
	const timer = setTimeout(() => ac.abort(), o.timeout);
	try {
		return await fetch(url, { headers, signal: ac.signal });
	} catch (e) {
		outer?.removeEventListener('abort', pass);
		throw e;
	} finally {
		clearTimeout(timer);
	}
}

export async function bytesOf(url: string, o: FetchOptions = {}): Promise<Uint8Array> {
	const r = await get(url, o);
	return new Uint8Array(await r.arrayBuffer());
}

export async function textOf(url: string, o: FetchOptions = {}): Promise<{ text: string; url: string }> {
	const r = await get(url, o);
	return { text: await r.text(), url: r.url || url };
}

/** Random access to a file's bytes. */
export interface ByteSource {
	/** Bytes in the file; NaN until known. */
	size: number;
	bytes(offset: number, length: number): Promise<Uint8Array>;
	prefetch(offset: number): void;
	close(): void;
}

export class RangeSource implements ByteSource {
	/** Bytes in the file; NaN until known. */
	size = NaN;
	private chunks = new Map<number, Uint8Array>();
	private pending = new Map<number, Promise<Uint8Array>>();
	private abort = new AbortController();
	/** The server sent the whole file for a range request: reads are served from that. */
	private whole: Uint8Array | null = null;

	constructor(
		readonly url: string,
		private readonly chunk = 1 << 20,
		private readonly keep = 12,
	) {}

	async open() {
		await this.load(0);
	}

	close() {
		this.abort.abort();
		this.chunks.clear();
		this.pending.clear();
		this.whole = null;
	}

	/** Up to `length` bytes from `offset`; fewer at the end of the file. */
	async bytes(offset: number, length: number): Promise<Uint8Array> {
		if (this.whole) return this.whole.subarray(offset, offset + length);
		if (Number.isFinite(this.size)) length = Math.max(0, Math.min(length, this.size - offset));
		if (length <= 0) return new Uint8Array(0);
		const first = Math.floor(offset / this.chunk);
		const last = Math.floor((offset + length - 1) / this.chunk);
		if (first === last) {
			const c = await this.load(first);
			const at = offset - first * this.chunk;
			return c.subarray(at, at + length);
		}
		const out = new Uint8Array(length);
		let filled = 0;
		const parts = [];
		for (let i = first; i <= last; i++) parts.push(this.load(i));
		for (let i = first; i <= last; i++) {
			const c = await parts[i - first];
			const from = i === first ? offset - first * this.chunk : 0;
			const take = Math.min(c.length - from, length - filled);
			if (take <= 0) break;
			out.set(c.subarray(from, from + take), filled);
			filled += take;
		}
		return out.subarray(0, filled);
	}

	/** Starts fetching the chunk that holds `offset`, for reading ahead. */
	prefetch(offset: number) {
		if (this.whole || offset < 0 || (Number.isFinite(this.size) && offset >= this.size)) return;
		void this.load(Math.floor(offset / this.chunk)).catch(() => {});
	}

	private load(index: number): Promise<Uint8Array> {
		if (this.whole) return Promise.resolve(this.whole.subarray(index * this.chunk, (index + 1) * this.chunk));
		const have = this.chunks.get(index);
		if (have) {
			// Most recently used last.
			this.chunks.delete(index);
			this.chunks.set(index, have);
			return Promise.resolve(have);
		}
		const pending = this.pending.get(index);
		if (pending) return pending;
		const from = index * this.chunk;
		const to = from + this.chunk - 1;
		const p = (async () => {
			const r = await get(this.url, { range: [from, to], signal: this.abort.signal });
			const body = new Uint8Array(await r.arrayBuffer());
			if (r.status === 200) {
				// No ranges: the body is the whole file.
				this.whole = body;
				this.size = body.length;
				return body.subarray(from, to + 1);
			}
			const total = r.headers.get('Content-Range')?.match(/\/(\d+)\s*$/)?.[1];
			if (total) this.size = Number(total);
			return body;
		})();
		this.pending.set(index, p);
		return p.then(
			(body) => {
				this.pending.delete(index);
				if (!this.whole) {
					this.chunks.set(index, body);
					while (this.chunks.size > this.keep) this.chunks.delete(this.chunks.keys().next().value!);
				}
				return body;
			},
			(e) => {
				this.pending.delete(index);
				throw e;
			},
		);
	}
}

/** Resolves `ref` against `base`. */
export function resolve(ref: string, base: string) {
	try {
		return new URL(ref, base).href;
	} catch {
		return ref;
	}
}
