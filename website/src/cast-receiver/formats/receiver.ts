// The receiver's side of its own unpacking: which loads go through the unpacker (engine.ts),
// subtitle tracks inside the file, side-loaded PGS files, and audio switches in place. A load the
// unpacker can't open goes to CAF as it came.

import type { Report } from '../errors';
import { readExtras, type SubtitleFile } from '../protocol';
import { Unpacker } from './engine';
import { Subtitles, supSamples, type TypesetSink } from './subs';
import { bytesOf } from './source';

/** What CAF loads for a subtitle track the receiver draws itself. */
export const EMPTY_VTT = 'data:text/vtt;charset=utf-8,WEBVTT%0A%0A';

const IMAGE_FILE = /^(pgs|sup)$/i;

interface LoadRequest {
	currentTime?: number;
	media?: {
		contentId?: string;
		contentUrl?: string;
		contentType?: string;
		streamType?: string;
		duration?: number;
		hlsSegmentFormat?: string;
		hlsVideoSegmentFormat?: string;
		tracks?: { trackId: number; trackContentId?: string; trackContentType?: string }[];
	};
}

export class Formats {
	private unpacker: Unpacker | null = null;
	private side: Subtitles | null = null;
	private sideUrl: string | null = null;
	private token = 0;

	constructor(
		private readonly video: HTMLVideoElement,
		private readonly base: string,
		private readonly sink: () => TypesetSink | null,
		private readonly report: (r: Report) => void,
	) {}

	get active() {
		return this.unpacker != null;
	}

	/** For measuring and the checks. */
	get current() {
		return this.unpacker;
	}

	/** The LOAD interceptor's part: plays the load through the unpacker when it opens. */
	async load<R>(request: R): Promise<R> {
		const token = ++this.token;
		this.stop();
		const r = request as LoadRequest;
		const media = r?.media;
		if (!media) return request;
		const extras = readExtras(request);
		const url = media.contentUrl || extras.url || media.contentId;
		if (typeof url !== 'string' || !/^https?:/i.test(url)) return request;
		for (const s of extras.subtitles ?? []) {
			if (s.inside == null) continue;
			const t = media.tracks?.find((x) => x.trackId === s.id);
			if (t) {
				t.trackContentId = EMPTY_VTT;
				t.trackContentType = 'text/vtt';
			}
		}
		const on = extras.subtitle != null ? extras.subtitles?.find((s) => s.id === extras.subtitle) : undefined;
		try {
			const u = await Unpacker.open(this.video, {
				url,
				contentType: media.contentType,
				container: extras.container,
				start: r.currentTime ?? 0,
				audio: extras.audio?.index ?? null,
				text: on?.inside ?? null,
				base: this.base,
				sink: this.sink,
				report: (x) => this.report(x),
			});
			if (token !== this.token) {
				u.destroy();
				return request;
			}
			this.unpacker = u;
			media.contentUrl = u.url;
			media.contentType = u.tracks.some((t) => t.kind === 'video') ? 'video/mp4' : 'audio/mp4';
			media.streamType = 'BUFFERED';
			delete media.hlsSegmentFormat;
			delete media.hlsVideoSegmentFormat;
			if (Number.isFinite(u.duration) && u.duration > 0) media.duration = u.duration;
		} catch (e) {
			console.info('CAF plays this load as it is:', e instanceof Error ? e.message : e);
		}
		return request;
	}

	/**
	 * Shows `entry` when it is the receiver's to draw (a track inside the file, or a PGS file).
	 * False when it isn't: the caller shows it the standard way. Whatever the receiver drew is off.
	 */
	async showSubtitle(entry: SubtitleFile | null): Promise<boolean> {
		if (entry?.inside != null && this.unpacker) {
			this.hideSide();
			await this.unpacker.showText(entry.inside);
			return true;
		}
		await this.unpacker?.showText(null);
		if (entry && IMAGE_FILE.test(String(entry.format)) && typeof entry.url === 'string') {
			if (this.sideUrl === entry.url) return true;
			this.hideSide();
			this.sideUrl = entry.url;
			try {
				const bytes = await bytesOf(entry.url, { what: 'subtitle' });
				if (this.sideUrl !== entry.url) return true;
				const subs = new Subtitles(this.video, null);
				await subs.show({ id: -1, kind: 'text', codec: 'pgs', language: entry.language }, async () => []);
				subs.push(supSamples(bytes, -1));
				this.side = subs;
			} catch (e) {
				this.sideUrl = null;
				this.report({
					part: 'subtitles',
					codec: 'pgs',
					detail: `PGS file: ${e instanceof Error ? e.message : e}`,
					fetched: 'subtitle',
					status: (e as { status?: number })?.status ?? null,
					detailed: null,
					shaka: null,
					lost: false,
				});
			}
			return true;
		}
		this.hideSide();
		return false;
	}

	/** Typeset subtitles inside the file fell behind: plain cues instead. */
	plainText() {
		void this.unpacker?.plainText();
	}

	/** Switches the audio track in place. Null when the unpacker isn't playing. */
	switchAudio(language: string, name?: string): boolean | null {
		const u = this.unpacker;
		if (!u) return null;
		const has = u.tracks.some((t) => t.kind === 'audio' && t.language === language);
		if (has) void u.switchAudio(language, name);
		return has;
	}

	private hideSide() {
		this.side?.destroy();
		this.side = null;
		this.sideUrl = null;
	}

	stop() {
		this.unpacker?.destroy();
		this.unpacker = null;
		this.hideSide();
	}
}
