// DASH: a static MPD's first period, with SegmentTemplate (numbers or a timeline), SegmentList or
// SegmentBase (sidx). fMP4 segments and WebVTT side files. Dynamic MPDs and ContentProtection are
// left to CAF's own player.

import { Reader } from './bytes';
import { LeaveToCaf, bytesOf, resolve, textOf } from './source';
import { SegmentedStream, type Rendition, type Segment } from './segmented';

/** Seconds from an ISO 8601 duration (PT1H2M3.5S). */
export function isoDuration(s: string | null | undefined): number {
	if (!s) return NaN;
	const m = s.match(/P(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:([\d.]+)S)?)?/);
	if (!m) return NaN;
	return (Number(m[1] ?? 0) * 24 + Number(m[2] ?? 0)) * 3600 + Number(m[3] ?? 0) * 60 + Number(m[4] ?? 0);
}

function kids(el: Element, name: string): Element[] {
	return Array.from(el.children).filter((c) => c.localName === name);
}
function kid(el: Element | null | undefined, name: string): Element | null {
	return el ? (kids(el, name)[0] ?? null) : null;
}

function baseOf(el: Element | null, base: string): string {
	const b = kid(el, 'BaseURL')?.textContent?.trim();
	return b ? resolve(b, base) : base;
}

function fill(template: string, id: string, bandwidth: string, number: number, time: number): string {
	return template.replace(/\$(RepresentationID|Number|Time|Bandwidth)(?:%0(\d+)d)?\$|\$\$/g, (all, name, width) => {
		if (all === '$$') return '$';
		const v = name === 'RepresentationID' ? id : name === 'Bandwidth' ? bandwidth : String(name === 'Number' ? number : time);
		return width ? v.padStart(Number(width), '0') : v;
	});
}

function range(s: string | null): [number, number] | undefined {
	if (!s) return undefined;
	const [a, b] = s.split('-').map(Number);
	return [a, b];
}

/** The attribute on the element, or the nearest parent level that sets it. */
function inherit(levels: (Element | null)[], name: string): string | null {
	for (const l of levels) {
		const v = l?.getAttribute(name);
		if (v != null) return v;
	}
	return null;
}

async function segmentsOf(
	rep: Element,
	set: Element,
	period: Element,
	base: string,
	periodStart: number,
	periodDuration: number,
): Promise<Segment[]> {
	const id = rep.getAttribute('id') ?? '';
	const bw = rep.getAttribute('bandwidth') ?? '';
	const tpl = [kid(rep, 'SegmentTemplate'), kid(set, 'SegmentTemplate'), kid(period, 'SegmentTemplate')];
	if (tpl.some(Boolean)) {
		const scale = Number(inherit(tpl, 'timescale') ?? 1);
		const pto = Number(inherit(tpl, 'presentationTimeOffset') ?? 0);
		const startNumber = Number(inherit(tpl, 'startNumber') ?? 1);
		const media = inherit(tpl, 'media') ?? '';
		const initT = inherit(tpl, 'initialization');
		const init = initT ? { url: resolve(fill(initT, id, bw, 0, 0), base) } : undefined;
		const offset = periodStart - pto / scale;
		const timeline = tpl.map((t) => kid(t, 'SegmentTimeline')).find(Boolean);
		const out: Segment[] = [];
		if (timeline) {
			let t = 0;
			let n = startNumber;
			const s = kids(timeline, 'S');
			s.forEach((e, i) => {
				if (e.getAttribute('t') != null) t = Number(e.getAttribute('t'));
				const d = Number(e.getAttribute('d'));
				let r = Number(e.getAttribute('r') ?? 0);
				if (r < 0) {
					const next = s[i + 1]?.getAttribute('t');
					const end = next != null ? Number(next) : (periodDuration + pto / scale) * scale;
					r = Math.ceil((end - t) / d) - 1;
				}
				for (let k = 0; k <= r; k++) {
					out.push({ url: resolve(fill(media, id, bw, n, t), base), start: (t - pto) / scale + periodStart, duration: d / scale, epoch: 0, init, offset });
					t += d;
					n++;
				}
			});
		} else {
			const d = Number(inherit(tpl, 'duration') ?? 0) / scale;
			const count = d > 0 ? Math.ceil(periodDuration / d - 1e-6) : 0;
			for (let k = 0; k < count; k++) {
				const n = startNumber + k;
				const t = Math.round(k * d * scale + pto);
				out.push({ url: resolve(fill(media, id, bw, n, t), base), start: periodStart + k * d, duration: d, epoch: 0, init, offset });
			}
		}
		return out;
	}
	const list = kid(rep, 'SegmentList') ?? kid(set, 'SegmentList');
	if (list) {
		const scale = Number(list.getAttribute('timescale') ?? 1);
		const d = Number(list.getAttribute('duration') ?? 0) / scale;
		const ini = kid(list, 'Initialization');
		const init = ini ? { url: resolve(ini.getAttribute('sourceURL') ?? '', base), range: range(ini.getAttribute('range')) } : undefined;
		return kids(list, 'SegmentURL').map((u, k) => ({
			url: resolve(u.getAttribute('media') ?? '', base),
			range: range(u.getAttribute('mediaRange')),
			start: periodStart + k * d,
			duration: d,
			epoch: 0,
			init,
			offset: periodStart,
		}));
	}
	const sb = kid(rep, 'SegmentBase') ?? kid(set, 'SegmentBase');
	if (sb) {
		const index = range(sb.getAttribute('indexRange'));
		const ini = kid(sb, 'Initialization');
		const init = { url: base, range: range(ini?.getAttribute('range') ?? null) ?? (index ? [0, index[0] - 1] as [number, number] : undefined) };
		if (!index) return [];
		const sidx = await bytesOf(base, { range: index, what: 'playlist' });
		return sidxSegments(sidx, base, index[1] + 1, init, periodStart);
	}
	// A single file (WebVTT side file)
	return [{ url: base, start: periodStart, duration: periodDuration, epoch: 0, offset: periodStart }];
}

function sidxSegments(b: Uint8Array, url: string, after: number, init: Segment['init'], periodStart: number): Segment[] {
	const r = new Reader(b);
	r.skip(4);
	if (r.ascii(4) !== 'sidx') return [];
	const v = r.u8();
	r.skip(3 + 4);
	const scale = r.u32();
	let time = v === 0 ? r.u32() : r.u64();
	const first = v === 0 ? r.u32() : r.u64();
	r.skip(2);
	const n = r.u16();
	let pos = after + first;
	const out: Segment[] = [];
	for (let i = 0; i < n; i++) {
		const size = r.u32() & 0x7fffffff;
		const dur = r.u32();
		r.skip(4);
		out.push({ url, range: [pos, pos + size - 1], start: periodStart + time / scale, duration: dur / scale, epoch: 0, init, offset: periodStart });
		pos += size;
		time += dur;
	}
	return out;
}

export async function openDash(url: string, screenHeight: number): Promise<SegmentedStream> {
	const { text, url: finalUrl } = await textOf(url, { what: 'playlist' });
	const doc = new DOMParser().parseFromString(text, 'application/xml');
	const mpd = doc.documentElement;
	if (mpd.getAttribute('type') === 'dynamic') throw new LeaveToCaf('live');
	if (doc.getElementsByTagNameNS('*', 'ContentProtection').length) throw new LeaveToCaf('keys');
	const period = kid(mpd, 'Period');
	if (!period) throw new Error('no Period');
	const total = isoDuration(mpd.getAttribute('mediaPresentationDuration'));
	const periodStart = isoDuration(period.getAttribute('start')) || 0;
	const periodDuration = isoDuration(period.getAttribute('duration')) || total - periodStart;
	const base = baseOf(period, baseOf(mpd, finalUrl));

	let video: Rendition | null = null;
	const audio: Rendition[] = [];
	const text_: Rendition[] = [];
	for (const set of kids(period, 'AdaptationSet')) {
		const reps = kids(set, 'Representation');
		const type = (r: Element) => set.getAttribute('contentType') ?? (inherit([r, set], 'mimeType') ?? '').split('/')[0];
		if (!reps.length) continue;
		const kind = type(reps[0]);
		const setBase = baseOf(set, base);
		const lang = set.getAttribute('lang') ?? undefined;
		const role = kid(set, 'Role')?.getAttribute('value');
		if (kind === 'video') {
			const h = (r: Element) => Number(r.getAttribute('height') ?? set.getAttribute('height') ?? 0);
			const fits = reps.filter((r) => !h(r) || h(r) <= screenHeight * 1.05);
			const rep = (fits.length ? fits : reps).sort((a, b) => Number(b.getAttribute('bandwidth')) - Number(a.getAttribute('bandwidth')))[0];
			if (video && (video.bandwidth ?? 0) >= Number(rep.getAttribute('bandwidth'))) continue;
			video = {
				kind: 'video',
				segments: await segmentsOf(rep, set, period, baseOf(rep, setBase), periodStart, periodDuration),
				format: 'mp4',
				codecs: [inherit([rep, set], 'codecs') ?? ''],
				bandwidth: Number(rep.getAttribute('bandwidth') ?? 0),
				height: h(rep) || undefined,
			};
		} else if (kind === 'audio') {
			const rep = reps.sort((a, b) => Number(b.getAttribute('bandwidth')) - Number(a.getAttribute('bandwidth')))[0];
			audio.push({
				kind: 'audio',
				segments: await segmentsOf(rep, set, period, baseOf(rep, setBase), periodStart, periodDuration),
				format: 'mp4',
				codecs: [inherit([rep, set], 'codecs') ?? ''],
				language: lang,
				name: set.getAttribute('label') ?? undefined,
				default: role === 'main',
			});
		} else if (kind === 'text') {
			const rep = reps[0];
			const mime = inherit([rep, set], 'mimeType') ?? '';
			if (mime !== 'text/vtt') continue;
			text_.push({
				kind: 'text',
				segments: await segmentsOf(rep, set, period, baseOf(rep, setBase), periodStart, periodDuration),
				format: 'vtt',
				language: lang,
				name: set.getAttribute('label') ?? undefined,
				forced: role === 'forced-subtitle',
			});
		}
	}
	return new SegmentedStream(video, audio, text_, Number.isFinite(total) ? total : periodDuration);
}
