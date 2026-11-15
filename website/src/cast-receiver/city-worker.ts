// Draws Ready's city off the page's main thread. The page hands over its two canvases once; each
// drawing lands in the one it names, and the page starts the fade when told it is done. On a
// Chromecast a drawing on the main thread held up a frame for 50 to 70 ms.

import { drawCity, SCENE, type Moment } from './city';

export type CityRequest =
	| { type: 'canvases'; canvases: OffscreenCanvas[] }
	| { type: 'size'; k: number }
	| { type: 'draw'; i: number; m: Moment; id: number }
	| { type: 'release' };

export type CityReply = { type: 'drawn'; i: number; id: number };

let canvases: OffscreenCanvas[] = [];

self.onmessage = (e: MessageEvent<CityRequest>) => {
	const r = e.data;
	if (r.type === 'canvases') {
		canvases = r.canvases;
	} else if (r.type === 'size') {
		const w = Math.round(SCENE.W * r.k);
		const h = Math.round(SCENE.H * r.k);
		for (const c of canvases) {
			if (c.width !== w || c.height !== h) {
				c.width = w;
				c.height = h;
			}
			c.getContext('2d')?.setTransform(r.k, 0, 0, r.k, 0, 0);
		}
	} else if (r.type === 'draw') {
		const g = canvases[r.i]?.getContext('2d');
		if (!g) return;
		drawCity(g, r.m);
		(self as unknown as Worker).postMessage({ type: 'drawn', i: r.i, id: r.id } satisfies CityReply);
	} else if (r.type === 'release') {
		for (const c of canvases) {
			c.width = 0;
			c.height = 0;
		}
	}
};
