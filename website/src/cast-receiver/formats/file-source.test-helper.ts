// A ByteSource over a local file, for tests.
import type { ByteSource } from './source';

export class FileSource implements ByteSource {
	size: number;
	private data: Uint8Array;
	reads = 0;
	constructor(path: string) {
		this.data = new Uint8Array(require('node:fs').readFileSync(path));
		this.size = this.data.length;
	}
	async bytes(offset: number, length: number) {
		this.reads++;
		return this.data.subarray(offset, Math.min(this.size, offset + length));
	}
	prefetch() {}
	close() {}
}
