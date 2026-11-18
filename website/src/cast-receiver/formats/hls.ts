// HLS: the master playlist's variants and renditions, and each media playlist's segments. One
// variant is picked by bandwidth within the screen's size. Live playlists and playlists with
// EXT-X-KEY are left to CAF's own player (LeaveToCaf).

import { LeaveToCaf, resolve, textOf } from './source';
import { SegmentedStream, type Rendition, type Segment } from './segmented';

function attrs(line: string): Record<string, string> {
	const out: Record<string, string> = {};
	const body = line.slice(line.indexOf(':') + 1);
	for (const m of body.matchAll(/([A-Z0-9-]+)=("[^"]*"|[^,]*)/g)) out[m[1]] = m[2].replace(/^"|"$/g, '');
	return out;
}


interface Media {
	segments: Segment[];
	format: 'ts' | 'mp4' | 'vtt';
	duration: number;
}

export async function mediaPlaylist(url: string, signal?: AbortSignal): Promise<Media> {
	const { text, url: base } = await textOf(url, { signal, what: 'playlist' });
	const lines = text.split(/\r?\n/).map((l) => l.trim());
	if (!lines.some((l) => l.startsWith('#EXT-X-ENDLIST')) && !lines.some((l) => /#EXT-X-PLAYLIST-TYPE:VOD/.test(l)))
		throw new LeaveToCaf('live');
	const segments: Segment[] = [];
	let start = 0;
	let duration = 0;
	let epoch = 0;
	let range: [number, number] | undefined;
	let lastEnd = 0;
	let init: Segment['init'];
	let format: Media['format'] = 'ts';
	for (const line of lines) {
		if (line.startsWith('#EXTINF:')) duration = parseFloat(line.slice(8));
		else if (line.startsWith('#EXT-X-DISCONTINUITY')) epoch++;
		else if (line.startsWith('#EXT-X-KEY')) {
			if (attrs(line).METHOD && attrs(line).METHOD !== 'NONE') throw new LeaveToCaf('keys');
		} else if (line.startsWith('#EXT-X-BYTERANGE:')) {
			const [len, off] = line.slice(17).split('@').map(Number);
			const from = Number.isFinite(off) ? off : lastEnd;
			range = [from, from + len - 1];
			lastEnd = from + len;
		} else if (line.startsWith('#EXT-X-MAP:')) {
			const a = attrs(line);
			let r: [number, number] | undefined;
			if (a.BYTERANGE) {
				const [len, off] = a.BYTERANGE.split('@').map(Number);
				r = [off || 0, (off || 0) + len - 1];
			}
			init = { url: resolve(a.URI, base), range: r };
			format = 'mp4';
		} else if (line && !line.startsWith('#')) {
			const url = resolve(line, base);
			if (/\.(vtt|webvtt)(\?|$)/i.test(line)) format = 'vtt';
			else if (!init && /\.(m4s|mp4|cmf[va]?)(\?|$)/i.test(line)) format = 'mp4';
			segments.push({ url, range, start, duration, epoch, init });
			start += duration;
			range = undefined;
		}
	}
	return { segments, format, duration: start };
}

/** Opens an HLS playlist (master or media) as one stream. */
export async function openHls(url: string, screenHeight: number): Promise<SegmentedStream> {
	const { text, url: base } = await textOf(url, { what: 'playlist' });
	if (!/#EXT-X-STREAM-INF/.test(text)) {
		const media = await mediaPlaylist(url);
		const r: Rendition = { kind: 'video', segments: media.segments, format: media.format, muxed: true };
		return new SegmentedStream(r, [], [], media.duration);
	}
	const lines = text.split(/\r?\n/).map((l) => l.trim());
	const variants: (Record<string, string> & { url: string })[] = [];
	const media: Record<string, string>[] = [];
	for (let i = 0; i < lines.length; i++) {
		const l = lines[i];
		if (l.startsWith('#EXT-X-STREAM-INF:')) {
			const a = attrs(l);
			let j = i + 1;
			while (j < lines.length && (!lines[j] || lines[j].startsWith('#'))) j++;
			if (j < lines.length) variants.push({ ...a, url: resolve(lines[j], base) });
		} else if (l.startsWith('#EXT-X-MEDIA:')) media.push(attrs(l));
		else if (l.startsWith('#EXT-X-SESSION-KEY')) throw new LeaveToCaf('keys');
	}
	// Variants with video, highest bandwidth that fits the screen; audio-only ones only when
	// nothing else exists.
	const withVideo = variants.filter((v) => v.RESOLUTION || /avc|hvc|hev|av01|vp0?9/.test(v.CODECS ?? ''));
	const pool = withVideo.length ? withVideo : variants;
	const height = (v: Record<string, string>) => Number(v.RESOLUTION?.split('x')[1] ?? 0);
	const fits = pool.filter((v) => !height(v) || height(v) <= screenHeight * 1.05);
	const choice = (fits.length ? fits : pool).sort((a, b) => Number(b.BANDWIDTH ?? 0) - Number(a.BANDWIDTH ?? 0))[0];
	if (!choice) throw new Error('no variants');

	const codecs = (choice.CODECS ?? '').split(',').map((c) => c.trim()).filter(Boolean);
	const audioGroup = choice.AUDIO;
	const textGroup = choice.SUBTITLES;
	const vm = await mediaPlaylist(choice.url);
	const video: Rendition = {
		kind: 'video',
		segments: vm.segments,
		format: vm.format,
		codecs,
		bandwidth: Number(choice.BANDWIDTH ?? 0),
		height: height(choice) || undefined,
		muxed: !audioGroup || !media.some((m) => m.TYPE === 'AUDIO' && m['GROUP-ID'] === audioGroup && m.URI),
	};
	const audio: Rendition[] = [];
	for (const m of media) {
		if (m.TYPE !== 'AUDIO' || m['GROUP-ID'] !== audioGroup || !m.URI) continue;
		const am = await mediaPlaylist(resolve(m.URI, base));
		audio.push({
			kind: 'audio',
			segments: am.segments,
			format: am.format,
			language: m.LANGUAGE,
			name: m.NAME,
			default: m.DEFAULT === 'YES',
			codecs,
		});
	}
	const text_: Rendition[] = [];
	for (const m of media) {
		if (m.TYPE !== 'SUBTITLES' || (textGroup && m['GROUP-ID'] !== textGroup) || !m.URI) continue;
		try {
			const tm = await mediaPlaylist(resolve(m.URI, base));
			text_.push({ kind: 'text', segments: tm.segments, format: 'vtt', language: m.LANGUAGE, name: m.NAME, default: m.DEFAULT === 'YES', forced: m.FORCED === 'YES' });
		} catch {
			// A subtitle playlist that doesn't load leaves the episode without it.
		}
	}
	return new SegmentedStream(video, audio, text_, vm.duration);
}
