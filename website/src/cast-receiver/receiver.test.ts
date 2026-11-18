import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { CLEAR, IDLE, cadence, drift, momentAt, skyStyle, stepOf, windowLevel } from './city';
import { classify, codecLabel, codecPart, refusedOf } from './errors';
import { FrameClock, BEHIND_SECONDS } from './frame-clock';
import { Strings, loadStrings, pickLanguage, type Index, type Pack } from './i18n';
import { asDevice, deviceFromAgent, message, readExtras, typesetFiles } from './protocol';
import { debugOn } from './debug';
import { SilentPicture } from './silent';
import { staysWithoutSenders } from './player';

const names = { 104: 'LOAD_FAILED', 301: 'MEDIA_SRC_NOT_SUPPORTED', 311: 'HLS_NETWORK_MASTER_PLAYLIST', 315: 'HLS_NETWORK_KEY_LOAD', 905: 'SEGMENT_NETWORK', 102: 'MEDIA_DECODE' };

describe('errors', () => {
	test('a refused audio codec blames the audio, never the video', () => {
		const r = classify({ detailedErrorCode: 102, error: { message: 'CHUNK_DEMUXER_ERROR_APPEND_FAILED: ec-3 not supported' } }, names);
		expect(r.part).toBe('audio');
		expect(r.codec).toBe('ec-3');
	});

	test('a failing video buffer blames the video', () => {
		const r = classify({ detailedErrorCode: 102, error: { shakaError: { code: 3016, data: ['video', 'decode'] } } }, names);
		expect(r.part).toBe('video');
		expect(r.codec).toBeNull();
	});

	test('a media error that names nothing stays unknown', () => {
		expect(classify({ detailedErrorCode: 102 }, names).part).toBe('unknown');
	});

	test('a playlist that nothing answered is a lost manifest', () => {
		const r = classify({ error: { shakaErrorCode: 1002, shakaErrorData: ['http://relay.example/m.m3u8', {}, 0] } }, names);
		expect(r).toMatchObject({ part: 'manifest', fetched: 'playlist', status: 0, lost: true, shaka: 1002 });
	});

	test('a segment with a bad status is the network, not lost', () => {
		const r = classify({ error: { shakaErrorCode: 1001, shakaErrorData: ['http://relay.example/s1.ts', 404, '', {}, 1] } }, names);
		expect(r).toMatchObject({ part: 'network', fetched: 'segment', status: 404, lost: false });
	});

	test('CAF names map without Shaka', () => {
		expect(classify({ detailedErrorCode: 311 }, names).part).toBe('manifest');
		expect(classify({ detailedErrorCode: 905 }, names)).toMatchObject({ part: 'network', fetched: 'segment', lost: true });
		expect(classify({ detailedErrorCode: 315 }, names).fetched).toBe('init');
	});

	test('text track errors are subtitles', () => {
		expect(classify({ error: { shakaErrorCode: 2004 } }, names)).toMatchObject({ part: 'subtitles', fetched: 'subtitle' });
	});

	test('a codec the TV refused, found after the failure, names the part', () => {
		expect(classify({ detailedErrorCode: 301 }, names, ['hvc1.2.4.L120.90']).part).toBe('video');
	});

	test("the engine's codec words, asked after a failure", () => {
		const tv = (type: string) => !/dtsc|hvc1\.2/.test(type);
		expect(refusedOf(['hevc-main10', 'dts'], tv)).toEqual(['hevc-main10', 'dts']);
		expect(refusedOf(['h264', 'dts'], tv)).toEqual(['dts']);
		expect(refusedOf(['video:mystery', null, undefined], tv)).toEqual([]);
		expect(classify({ detailedErrorCode: 102 }, names, refusedOf(['h264', 'dts'], tv))).toMatchObject({ part: 'audio', codec: 'dts' });
		expect(codecPart('hevc-main10')).toBe('video');
		expect(codecPart('truehd')).toBe('audio');
		expect(codecPart('opus')).toBe('audio');
		expect(codecPart('av01.0.08M.08')).toBe('video');
		expect(codecPart('wvtt')).toBeNull();
	});
});

describe('strings', () => {
	const dir = join(import.meta.dir, '../../public/cast/receiver/i18n');
	const read = (f: string) => JSON.parse(readFileSync(join(dir, f), 'utf8'));
	const index = read('index.json') as Index;

	test('language picks', () => {
		expect(pickLanguage(index.languages, 'pt-BR')).toBe('pt-BR');
		expect(pickLanguage(index.languages, 'pt-PT')).toBe('pt-BR');
		expect(pickLanguage(index.languages, 'es-ES')).toBe('es-419');
		expect(pickLanguage(index.languages, 'de_AT')).toBe('de');
		expect(pickLanguage(index.languages, 'xx')).toBeNull();
		expect(pickLanguage(index.languages, null)).toBeNull();
	});

	test('every language has every source message', () => {
		const source = read(`${index.source}.json`) as Pack;
		for (const tag of index.languages) {
			const pack = read(`${tag}.json`) as Pack;
			expect(Object.keys(pack.messages).sort()).toEqual(Object.keys(source.messages).sort());
		}
	});

	test('arguments, numbers and the fallback to the source', () => {
		const source = read('en.json') as Pack;
		const de = read('de.json') as Pack;
		const s = new Strings(index, { ...de, messages: { ...de.messages, 'receiver-paused': undefined as never } }, source);
		expect(s.t('receiver-waiting', { seconds: 1234 })).toContain('1.234');
		expect(s.t('receiver-error', { episode: '7' })).toContain('7');
		expect(s.t('receiver-paused')).toBe('Paused');
		expect(s.t('receiver-nothing')).toBe('receiver-nothing');
	});

	test('plurals choose the form for the count', () => {
		const pack: Pack = { tag: 'pl', messages: { n: { plural: 'n', forms: { one: ['one'], few: ['few'], many: ['many'], other: ['other'] } } } };
		const s = new Strings({ source: 'en', languages: ['pl'], args: {} }, pack, pack);
		expect([1, 3, 5].map((n) => s.t('n', { n }))).toEqual(['one', 'few', 'many']);
	});

	test('loading falls back to English for a language without strings', async () => {
		const fetcher = (async (url: string) => {
			const file = url.split('/i18n/')[1];
			return new Response(readFileSync(join(dir, file)));
		}) as unknown as typeof fetch;
		expect((await loadStrings('https://kaeri.example/cast/receiver/', 'ja-JP', fetcher)).tag).toBe('ja');
		expect((await loadStrings('https://kaeri.example/cast/receiver/', 'xx', fetcher)).tag).toBe('en');
	});
});

describe('frame clock', () => {
	test('five behind seconds in a row switch to plain subtitles', () => {
		let t = 0;
		const clock = new FrameClock(() => t);
		const second = (late: boolean) => {
			for (let i = 0; i < 24; i++) {
				clock._startFrame();
				t += 1;
				clock._endFrame({ expectedDisplayTime: late ? t - 80 : t });
			}
			return clock.tick(true);
		};
		for (let i = 1; i < BEHIND_SECONDS; i++) expect(second(true)).toBe(false);
		expect(second(false)).toBe(false); // a good second starts over
		for (let i = 1; i < BEHIND_SECONDS; i++) expect(second(true)).toBe(false);
		expect(second(true)).toBe(true);
	});

	test('dropped frames count, a paused video does not', () => {
		const clock = new FrameClock(() => 0);
		for (let s = 0; s < 10; s++) {
			for (let i = 0; i < 10; i++) clock._drop();
			expect(clock.tick(false)).toBe(false);
		}
	});
});

describe('silent picture', () => {
	test('sound for 3 s with no picture is reported once', () => {
		const s = new SilentPicture();
		const at = (time: number) => s.check({ time, videoWidth: 0, frames: 0, playing: true });
		expect([0, 1, 2].map(at)).toEqual([false, false, false]);
		expect(at(3)).toBe(true);
		expect(at(4)).toBe(false);
		s.reset();
		expect(at(5)).toBe(false);
	});

	test('frames that stop arriving are reported; frames that keep coming are not', () => {
		const s = new SilentPicture();
		for (let i = 0; i < 10; i++) expect(s.check({ time: i, videoWidth: 1920, frames: i * 24, playing: true })).toBe(false);
		expect(s.check({ time: 10, videoWidth: 1920, frames: 240, playing: true })).toBe(false);
		expect(s.check({ time: 12, videoWidth: 1920, frames: 240, playing: true })).toBe(false);
		expect(s.check({ time: 13, videoWidth: 1920, frames: 240, playing: true })).toBe(true);
	});

	test('shown frames count as picture where decoded frames are not counted', () => {
		const s = new SilentPicture();
		for (let i = 0; i < 10; i++) expect(s.check({ time: i, videoWidth: 1920, frames: 0, presented: i * 24, playing: true })).toBe(false);
		const stuck = (time: number) => s.check({ time, videoWidth: 1920, frames: 0, presented: 216, playing: true });
		expect([10, 11].map(stuck)).toEqual([false, false]);
		expect(stuck(12)).toBe(true);
	});

	test('a size alone is no picture when the TV counts frames', () => {
		const s = new SilentPicture();
		const at = (time: number) => s.check({ time, videoWidth: 720, frames: 0, presented: 0, playing: true });
		expect([0, 1, 2].map(at)).toEqual([false, false, false]);
		expect(at(3)).toBe(true);
	});

	test('a paused video is never silent', () => {
		const s = new SilentPicture();
		for (let i = 0; i < 10; i++) expect(s.check({ time: 0, videoWidth: 0, frames: 0, playing: false })).toBe(false);
	});
});

describe('debug status', () => {
	test('only the dev receiver or ?debug shows it', () => {
		const at = (url: string) => debugOn(new URL(url) as unknown as Location);
		expect(at('https://cast-dev.kaeri.app/')).toBe(true);
		expect(at('https://kaeri.app/cast/receiver/?debug')).toBe(true);
		expect(at('https://kaeri.app/cast/receiver/')).toBe(false);
	});
});

describe('protocol', () => {
	test('messages carry the version', () => {
		expect(message('subtitles-plain', { reason: 'slow' })).toEqual({ type: 'subtitles-plain', version: 1, reason: 'slow' });
	});

	test('typeset files are the ASS and SSA ones, with the fonts', () => {
		const files = typesetFiles({
			fonts: ['http://relay.example/f/1.ttf'],
			subtitles: [
				{ id: 1, url: 'http://relay.example/x/1.ass', format: 'ass', language: 'en' },
				{ id: 2, url: 'http://relay.example/x/2.srt', format: 'srt' },
				{ id: 3, url: 'http://relay.example/x/3.ssa', format: 'ssa', fonts: [] },
			],
		});
		expect(files).toEqual([
			{ trackId: 1, ass: 'http://relay.example/x/1.ass', fonts: ['http://relay.example/f/1.ttf'] },
			{ trackId: 3, ass: 'http://relay.example/x/3.ssa', fonts: [] },
		]);
	});

	test('Kaeri extras come from the media or the request', () => {
		expect(readExtras({ media: { customData: { kaeri: { episode: 7 } } } }).episode).toBe(7);
		expect(readExtras({ customData: { kaeri: { device: 'ipad' } } }).device).toBe('ipad');
		expect(readExtras({ media: {} })).toEqual({});
	});

	test('devices', () => {
		expect(asDevice('mac')).toBe('mac');
		expect(asDevice('toaster')).toBeNull();
		expect(deviceFromAgent('Mozilla/5.0 (iPad; CPU OS 26_0)')).toBe('ipad');
		expect(deviceFromAgent('Mozilla/5.0 (Linux; Android 16)')).toBe('phone');
		expect(deviceFromAgent('Mozilla/5.0 (Windows NT 10.0)')).toBe('computer');
	});
});

describe('codec names', () => {
	test('the ids the receiver reports get names people know', () => {
		expect(codecLabel('mp2v')).toBe('MPEG-2');
		expect(codecLabel('video:mpeg2')).toBe('MPEG-2');
		expect(codecLabel('hev1.1.6.L93.B0')).toBe('HEVC');
		expect(codecLabel('hvc1')).toBe('HEVC');
		expect(codecLabel('av01.0.05M.08')).toBe('AV1');
		expect(codecLabel('video:h.264 high 10')).toBe('H.264 High 10');
		expect(codecLabel('avc1.6e001f')).toBe('H.264 High 10');
		expect(codecLabel('ac-3')).toBe('Dolby Digital');
		expect(codecLabel('ec-3')).toBe('Dolby Digital Plus');
		expect(codecLabel('truehd')).toBe('Dolby TrueHD');
	});
	test('an id without a name leaves the line out', () => {
		expect(codecLabel('video:qx9')).toBeNull();
		expect(codecLabel(null)).toBeNull();
		expect(codecLabel('')).toBeNull();
	});
});

describe('city at dusk', () => {
	test('the drift goes 36 px out and back over 24 min in whole pixels', () => {
		expect(drift(0)).toEqual([0, 0]);
		expect(drift(360)).toEqual([36, -18]);
		expect(drift(720)).toEqual([0, -36]);
		expect(drift(1440)).toEqual([0, 0]);
		const [x, y] = drift(100);
		expect(Number.isInteger(x) && Number.isInteger(y)).toBe(true);
	});
	test('dusk turns to night over the ten minutes and the windows go out in the last 30 s', () => {
		expect(momentAt(0, false).dusk).toBe(0);
		expect(momentAt(IDLE, false).dusk).toBe(1);
		expect(momentAt(560, false).out).toBe(0);
		expect(momentAt(IDLE - 4, false).out).toBe(1);
		let lit = 0;
		let litAtEnd = 0;
		for (let id = 1; id < 400; id++) {
			if (windowLevel(id, 300, momentAt(300, false)) > 0) lit++;
			if (windowLevel(id, IDLE - 4, momentAt(IDLE - 4, false)) > 0) litAtEnd++;
		}
		expect(lit).toBeGreaterThan(100);
		expect(litAtEnd).toBe(0);
	});
	test('no window lights before the launch reveals it', () => {
		for (let id = 1; id < 200; id++) expect(windowLevel(id, 0, { reveal: 0, out: 0 })).toBe(0);
	});
	test('the sky is three CSS backgrounds with the last light gone by night', () => {
		expect(skyStyle(0).match(/gradient\(/g)?.length).toBe(3);
		expect(skyStyle(1)).toContain('rgba(242,166,75,0)');
	});
	test('the city is drawn a step at a time: every 5 s, every 2 min under Reduce Motion', () => {
		expect(cadence(100, false).every).toBe(5);
		expect(cadence(100, true)).toEqual({ every: 120, fade: 0.36 });
		expect(stepOf(123.4, false)).toBe(120);
		expect(stepOf(239, true)).toBe(120);
		expect(cadence(580, false).every).toBe(2);
	});
	test('the clear area holds the name and the ready line with their drift', () => {
		// The text sits at 96, 84 and drifts up to 36 px each way.
		expect(CLEAR.x0).toBeLessThanOrEqual(96 - 36);
		expect(CLEAR.y0).toBeLessThanOrEqual(84 - 36);
		expect(CLEAR.y1).toBeGreaterThanOrEqual(84 + 80 + 18 + 34 + 18);
	});
});

describe('senders', () => {
	test('an episode on screen stays up when the last sender leaves; Ready and an error close', () => {
		for (const phase of ['loading', 'playing', 'gone', 'finished']) expect(staysWithoutSenders(phase)).toBe(true);
		for (const phase of ['idle', 'error']) expect(staysWithoutSenders(phase)).toBe(false);
	});
});
