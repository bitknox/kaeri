// Ready's scene, the city at dusk: one soft gradient sky going from dusk to night over the ten
// idle minutes, stars and a moon kept clear of the text, and two low skylines with lit windows
// that switch slowly. Against burn-in the city pans: the near skyline 24 px every 2 min, the
// far one 10, the stars 4. In the last 30 s the windows go out.
//
// Kept light for a TV: nothing runs an animation of its own. The sky is a CSS gradient restyled
// a step at a time. Stars, moon and skylines are drawn into one of two canvases every few
// seconds, and the new one fades in over the old with an opacity transition, so the compositor
// does the in-between frames. The canvases are let go while the city is hidden.

export const SCENE = { W: 1280, H: 720, horizon: 590 };
/** The idle minutes, and when the closing starts. */
export const IDLE = 600;
export const CLOSING = 570;

type RGB = [number, number, number];

const hash = (n: number) => (((Math.sin(n) * 43758.5453) % 1) + 1) % 1;
const pmod = (n: number, m: number) => ((n % m) + m) % m;
const clamp = (v: number, a = 0, b = 1) => Math.max(a, Math.min(b, v));
const mix = (a: RGB, b: RGB, k: number): RGB => [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k, a[2] + (b[2] - a[2]) * k];
const rgb = (c: RGB, a = 1) => `rgba(${Math.round(c[0])},${Math.round(c[1])},${Math.round(c[2])},${a})`;

function ramp(stops: [number, RGB][], t: number): RGB {
	const at = clamp(t);
	for (let i = 1; i < stops.length; i++) {
		if (at <= stops[i][0]) {
			const [t0, c0] = stops[i - 1];
			const [t1, c1] = stops[i];
			return mix(c0, c1, (at - t0) / (t1 - t0));
		}
	}
	return stops[stops.length - 1][1];
}

const SKY_DUSK: [number, RGB][] = [[0, [16, 19, 31]], [0.42, [34, 36, 72]], [0.7, [86, 62, 104]], [0.88, [170, 104, 104]], [1, [232, 150, 104]]];
const SKY_NIGHT: [number, RGB][] = [[0, [9, 11, 20]], [0.5, [16, 19, 31]], [0.85, [26, 30, 58]], [1, [40, 44, 78]]];
const AMBER: RGB = [242, 166, 75];
const WATER: RGB = [127, 199, 217];
const SILK: RGB = [228, 224, 214];

/** The name and the ready line, with their drift: no star or moon ever passes here. */
export const CLEAR = { x0: 56, x1: 980, y0: 36, y1: 262 };

interface Star {
	x: number;
	y: number;
	r: number;
	a: number;
	th: number;
	tw: number;
}
const STARS: Star[] = (() => {
	const out: Star[] = [];
	for (let i = 0; i < 170; i++) {
		const x = hash(i * 12.9898 + 1.3) * 1480;
		const y = 14 + hash(i * 78.233 + 4.7) * 380;
		out.push({ x, y, r: 0.7 + 1.1 * hash(i * 3.7 + 2) ** 2, a: 0.35 + 0.6 * hash(i * 5.1 + 9), th: hash(i * 9.3 + 7) * 0.8, tw: 3 + 6 * hash(i * 2.3) });
	}
	return out;
})();

interface Block {
	x: number;
	w: number;
	h: number;
	i: number;
	roof: number;
}
function skyline(seed: number, x0: number, x1: number, wMin: number, wMax: number, hMin: number, hMax: number, gapP: number): Block[] {
	const out: Block[] = [];
	let x = x0;
	let i = 0;
	while (x < x1) {
		const w = wMin + (wMax - wMin) * hash(seed + i * 7.31);
		const h = hMin + (hMax - hMin) * hash(seed + i * 3.17 + 0.5) ** 1.3;
		if (hash(seed + i * 5.9) > gapP) out.push({ x, w: Math.round(w), h: Math.round(h), i, roof: hash(seed + i * 2.2) });
		x += w + 6 + 20 * hash(seed + i * 9.1);
		i++;
	}
	return out;
}
const FAR = skyline(11, -160, 1620, 40, 110, 50, 190, 0.1);
const NEAR = skyline(37, -200, 1660, 70, 160, 40, 200, 0.2);

/** What the city looks like at idle time `t`. */
export interface Moment {
	t: number;
	/** 0 at dusk, 1 at night. */
	dusk: number;
	/** The share of windows lit while the city first appears. */
	reveal: number;
	/** How far the windows have gone out at the end. */
	out: number;
	still: boolean;
}

export function momentAt(t: number, still: boolean, reveal = 1): Moment {
	return { t, dusk: clamp(t / IDLE), reveal, out: t > CLOSING ? clamp((t - CLOSING) / 26) : 0, still };
}

/** A window's light, 0 to 1: each switches on or off now and then, over 1.4 s. */
export function windowLevel(id: number, t: number, m: Pick<Moment, 'reveal' | 'out'>): number {
	const P = 80 + 70 * hash(id * 1.7);
	const ph = P * hash(id * 4.1 + 0.3);
	const k = Math.floor((t + ph) / P);
	const on = (j: number) => (hash(id * 13.37 + j * 1.618) > 0.42 ? 1 : 0);
	const f = clamp((t + ph - k * P) / 1.4);
	let v = on(k - 1) + (on(k) - on(k - 1)) * f;
	if (m.reveal < 1 && hash(id * 2.9 + 5.3) >= m.reveal) v = 0;
	if (m.out > 0) v *= clamp((hash(id * 1.7 + 2.3) - m.out) * 8);
	return v;
}

/** The sky as CSS backgrounds: the last light low in the west, haze on the horizon, the gradient. */
export function skyStyle(dusk: number): string {
	const H = SCENE.horizon;
	const qs = [...new Set([...SKY_DUSK.map((s) => s[0]), ...SKY_NIGHT.map((s) => s[0])])].sort((a, b) => a - b);
	const stops = qs.map((q) => `${rgb(mix(ramp(SKY_DUSK, q), ramp(SKY_NIGHT, q), dusk))} ${Math.round(q * H)}px`).join(', ');
	const glow = (1 - dusk) * 0.55;
	const haze = mix([206, 130, 110], [44, 48, 84], dusk);
	return [
		`radial-gradient(circle 620px at 980px ${H + 30}px, ${rgb(AMBER, Math.round(glow * 1000) / 1000)}, ${rgb(AMBER, 0)})`,
		`linear-gradient(to bottom, ${rgb(haze, 0)} ${H - 150}px, ${rgb(haze, 0.35)} ${H + 10}px, ${rgb(haze, 0)} ${H + 10}px)`,
		`linear-gradient(to bottom, ${stops})`,
	].join(', ');
}

type Ctx = CanvasRenderingContext2D;
function rounded(g: Ctx, x: number, y: number, w: number, h: number, r: number) {
	g.beginPath();
	if (typeof g.roundRect === 'function') g.roundRect(x, y, w, h, r);
	else g.rect(x, y, w, h);
	g.fill();
}

/** Stars, the moon and both skylines, over a clear canvas (the sky shows through). */
export function drawCity(g: Ctx, m: Moment) {
	const { W, H, horizon } = SCENE;
	const t = m.t;
	const dusk = m.dusk;
	g.clearRect(0, 0, W, H);

	// Stars come out as it darkens.
	const sx = -t * 0.03;
	for (const s of STARS) {
		const vis = clamp((dusk + 0.25 - s.th) * 3);
		if (vis <= 0) continue;
		const x = pmod(s.x + sx, 1480) - 100;
		const y = s.y;
		if (x > CLEAR.x0 && x < CLEAR.x1 && y > CLEAR.y0 && y < CLEAR.y1) continue;
		const tw = m.still ? 1 : 0.75 + 0.25 * Math.sin((t / s.tw) * Math.PI * 2 + s.x);
		g.fillStyle = rgb(SILK, s.a * vis * tw);
		g.beginPath();
		g.arc(x, y, s.r, 0, Math.PI * 2);
		g.fill();
	}

	// The moon climbs over the ten minutes, right of the text.
	const mu = clamp(t / IDLE);
	const mx = 1100 - 80 * mu;
	const my = 300 - 150 * mu;
	const mv = clamp(dusk * 1.6 + 0.15);
	const halo = g.createRadialGradient(mx, my, 10, mx, my, 150);
	halo.addColorStop(0, rgb(SILK, 0.16 * mv));
	halo.addColorStop(1, rgb(SILK, 0));
	g.fillStyle = halo;
	g.fillRect(mx - 150, my - 150, 300, 300);
	g.fillStyle = rgb([232, 226, 212], 0.92 * mv);
	g.beginPath();
	g.arc(mx, my, 22, 0, Math.PI * 2);
	g.fill();

	// Far blocks: low contrast, few lights.
	const fx = -t * 0.08;
	g.fillStyle = rgb(mix([70, 58, 100], [26, 30, 56], dusk));
	for (const b of FAR) {
		const x = b.x + fx;
		if (x > W || x + b.w < 0) continue;
		g.fillRect(x, horizon - b.h, b.w, b.h + 200);
	}
	for (const b of FAR) {
		const x = b.x + fx;
		if (x > W || x + b.w < 0) continue;
		for (let wy = horizon - b.h + 12; wy < horizon - 6; wy += 16) {
			for (let wx = 8; wx < b.w - 8; wx += 14) {
				const id = b.i * 977 + wy * 3 + wx;
				if (hash(id * 0.71) < 0.78) continue;
				const v = windowLevel(id, t, m);
				if (v <= 0.01) continue;
				g.fillStyle = rgb(hash(id) < 0.85 ? AMBER : WATER, 0.45 * v);
				rounded(g, x + wx, wy, 3.5, 4.5, 1);
			}
		}
	}

	// Near towers: darker, taller, with warm windows that glow.
	const nearC = rgb(mix([38, 36, 66], [16, 19, 31], dusk));
	const edgeC = rgb(mix([110, 84, 104], [52, 58, 92], dusk), 0.6);
	const nx = -t * 0.2;
	for (const b of NEAR) {
		const x = b.x + nx;
		if (x > W || x + b.w < 0) continue;
		const top = H - 70 - b.h;
		g.fillStyle = nearC;
		if (b.roof > 0.7) {
			g.beginPath();
			g.moveTo(x, top + 14);
			g.lineTo(x + b.w * 0.5, top - 10);
			g.lineTo(x + b.w, top + 14);
			g.lineTo(x + b.w, H);
			g.lineTo(x, H);
			g.closePath();
			g.fill();
		} else {
			g.fillRect(x, top, b.w, H - top);
			if (b.roof < 0.25) g.fillRect(x + b.w * 0.62, top - 30, 2, 30);
		}
		g.fillStyle = edgeC;
		g.fillRect(x, b.roof > 0.7 ? top + 14 : top, 1.5, H);
		for (let wy = top + 26; wy < H - 24; wy += 24) {
			for (let wx = 12; wx < b.w - 14; wx += 20) {
				const id = 50000 + b.i * 977 + wy * 3 + wx;
				if (hash(id * 0.37) < 0.5) continue;
				const v = windowLevel(id, t, m);
				if (v <= 0.01) continue;
				const col = hash(id) < 0.86 ? AMBER : WATER;
				const lum = (0.55 + 0.45 * hash(id * 3.3)) * v;
				g.fillStyle = rgb(col, 0.1 * lum);
				rounded(g, x + wx - 5, wy - 5, 16, 18, 6);
				g.fillStyle = rgb(mix(col, [255, 236, 200], 0.2), 0.85 * lum);
				rounded(g, x + wx, wy, 6, 8, 1.5);
			}
		}
	}

	// A soft floor of night at the bottom, so the city sits on the ground.
	const fl = g.createLinearGradient(0, H - 120, 0, H);
	fl.addColorStop(0, rgb([16, 19, 31], 0));
	fl.addColorStop(1, rgb([16, 19, 31], 0.85));
	g.fillStyle = fl;
	g.fillRect(0, H - 120, W, 120);
}

/** How often the city is drawn again, and how long the new drawing fades in, in seconds. */
export function cadence(t: number, still: boolean): { every: number; fade: number } {
	if (t >= CLOSING) return { every: 2, fade: 1.4 };
	if (still) return { every: 120, fade: 0.36 };
	return { every: 5, fade: 1.4 };
}

/** The idle time a drawing shows: whole steps, so every window and pan lands on the step. */
export function stepOf(t: number, still: boolean): number {
	const { every } = cadence(t, still);
	return Math.floor(t / every) * every;
}

/** The slow drift against burn-in: 36 px out and back over 24 min, a whole pixel at a time. */
export function drift(seconds: number): [number, number] {
	const P = [
		[0, 0],
		[36, -18],
		[0, -36],
		[-36, -18],
		[0, 0],
	];
	const u = pmod(seconds, 1440) / 360;
	const i = Math.min(3, Math.floor(u));
	const f = u - i;
	return [Math.round(P[i][0] + (P[i + 1][0] - P[i][0]) * f), Math.round(P[i][1] + (P[i + 1][1] - P[i][1]) * f)];
}

const REVEAL_FROM = 0.2;
const REVEAL_FOR = 1.6;

export class City {
	private sky: HTMLElement;
	private canvases: HTMLCanvasElement[];
	private front = 0;
	private drawn: string | null = null;
	private skyKey = -1;
	private fadeUntil = 0;
	private shown = false;
	private releaseTimer: ReturnType<typeof setTimeout> | null = null;
	private revealStart: number | null = null;
	private revealTimer: ReturnType<typeof setTimeout> | null = null;
	/** Draws, for the debug status and the checks. */
	draws = 0;

	constructor(
		private root: HTMLElement,
		private clock: () => number = () => performance.now(),
	) {
		this.sky = root.querySelector('.sky') as HTMLElement;
		this.canvases = [...root.querySelectorAll('canvas')] as HTMLCanvasElement[];
	}

	/**
	 * Shows the city at idle time `t` (seconds), or keeps it current. `reveal` lights the windows
	 * one by one over 1.6 s, as at launch.
	 */
	show(t: number, still: boolean, reveal = false) {
		if (this.releaseTimer) clearTimeout(this.releaseTimer);
		this.releaseTimer = null;
		if (!this.shown) {
			this.shown = true;
			this.allocate();
			this.revealStart = reveal ? this.clock() : null;
			this.drawn = null;
			this.paint(t, still, true);
			if (reveal) this.revealTick(t, still);
			return;
		}
		if (this.revealStart != null) return;
		this.paint(t, still, false);
	}

	/** Hides the city; its canvases are let go once the fade is over. */
	hide() {
		if (!this.shown) return;
		this.shown = false;
		this.revealStart = null;
		if (this.revealTimer) clearTimeout(this.revealTimer);
		this.revealTimer = null;
		this.releaseTimer = setTimeout(() => this.release(), 600);
	}

	get visible() {
		return this.shown;
	}

	private allocate() {
		const k = this.scale();
		for (const c of this.canvases) {
			const w = Math.round(SCENE.W * k);
			const h = Math.round(SCENE.H * k);
			if (c.width !== w || c.height !== h) {
				c.width = w;
				c.height = h;
			}
			c.getContext('2d')?.setTransform(k, 0, 0, k, 0, 0);
			c.style.transition = 'none';
			c.style.opacity = '0';
		}
	}

	private release() {
		this.releaseTimer = null;
		for (const c of this.canvases) {
			c.width = 0;
			c.height = 0;
		}
		this.drawn = null;
		this.skyKey = -1;
	}

	/** Device pixels per scene pixel: the TV's 1.5 at most. */
	private scale(): number {
		const s = Number(getComputedStyle(this.root.parentElement ?? this.root).getPropertyValue('--s')) || 1;
		return Math.min(1.5, Math.max(0.5, s * (devicePixelRatio || 1)));
	}

	private revealTick(t: number, still: boolean) {
		const start = this.revealStart;
		if (start == null || !this.shown) return;
		const e = (this.clock() - start) / 1000;
		const r = still ? 1 : clamp((e - REVEAL_FROM) / REVEAL_FOR);
		this.drawInto(this.front, momentAt(stepOf(t, still), still, r));
		if (r >= 1) {
			this.revealStart = null;
			return;
		}
		this.revealTimer = setTimeout(() => this.revealTick(t, still), 160);
	}

	private paint(t: number, still: boolean, first: boolean) {
		const step = stepOf(t, still);
		const dusk = clamp(step / IDLE);
		const skyKey = Math.round(dusk * 120);
		if (skyKey !== this.skyKey) {
			this.skyKey = skyKey;
			this.sky.style.background = skyStyle(skyKey / 120);
		}
		const m = momentAt(step, still, first && this.revealStart != null ? 0 : 1);
		const key = `${step}|${still}`;
		if (key === this.drawn) return;
		const now = this.clock();
		if (!first && now < this.fadeUntil) return; // the last fade is still running
		this.drawn = key;
		if (first) {
			this.drawInto(this.front, m);
			const c = this.canvases[this.front];
			c.style.transition = 'none';
			c.style.opacity = '1';
			this.canvases[1 - this.front].style.opacity = '0';
			return;
		}
		// The new drawing fades in over the old one, then takes its place.
		const back = 1 - this.front;
		this.drawInto(back, m);
		const { fade } = cadence(t, still);
		const b = this.canvases[back];
		const f = this.canvases[this.front];
		b.style.zIndex = '2';
		f.style.zIndex = '1';
		b.style.transition = 'none';
		b.style.opacity = '0';
		void b.offsetWidth;
		b.style.transition = `opacity ${fade * 1000}ms ease-in-out`;
		b.style.opacity = '1';
		this.fadeUntil = now + fade * 1000 + 50;
		const was = this.front;
		this.front = back;
		setTimeout(() => {
			if (this.front !== was) {
				const old = this.canvases[was];
				old.style.transition = 'none';
				old.style.opacity = '0';
			}
		}, fade * 1000 + 50);
	}

	private drawInto(i: number, m: Moment) {
		const g = this.canvases[i].getContext('2d');
		if (!g) return;
		drawCity(g, m);
		this.draws++;
	}
}
