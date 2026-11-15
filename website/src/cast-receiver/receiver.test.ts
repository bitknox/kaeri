import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { classify, codecPart, codecsInMaster } from './errors';
import { FrameClock, BEHIND_SECONDS } from './frame-clock';
import { Strings, loadStrings, pickLanguage, type Index, type Pack } from './i18n';
import { asDevice, deviceFromAgent, message, readExtras } from './protocol';
import { SilentPicture } from './silent';

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

	test('codecs from an HLS master', () => {
		const master = '#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,CODECS="avc1.640028,mp4a.40.2"\nv.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=2,CODECS="hvc1.1.6.L120.90,mp4a.40.2"\nh.m3u8\n';
		expect(codecsInMaster(master)).toEqual(['avc1.640028', 'mp4a.40.2', 'hvc1.1.6.L120.90']);
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

	test('a paused video is never silent', () => {
		const s = new SilentPicture();
		for (let i = 0; i < 10; i++) expect(s.check({ time: 0, videoWidth: 0, frames: 0, playing: false })).toBe(false);
	});
});

describe('protocol', () => {
	test('messages carry the version', () => {
		expect(message('subtitles-plain', { reason: 'slow' })).toEqual({ type: 'subtitles-plain', version: 1, reason: 'slow' });
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
