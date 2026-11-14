// Draws the terminal app's golden screens as SVG for the site: bun scripts/terminal-shots.ts
//
// The goldens in crates/kaeri-tui/tests/goldens are what the terminal draws, cell for cell, with
// each cell's colour role. This reads them and writes public/terminal/<screen>-<cols>.svg: text
// as text, block and line characters as shapes so rows meet without gaps, and the cover plate
// filled with the fixture art, the way a terminal with a graphics protocol shows it.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '..', '..');
const goldens = join(repo, 'crates/kaeri-tui/tests/goldens');
const art = join(repo, 'crates/kaeri-testkit/fixtures/art');
const out = join(here, '..', 'public/terminal');

// Each screen, and the fixture art its plate shows (the art the native Fixtures apps use for
// the same title), if it has one.
const screens: { name: string; golden: string; art?: string }[] = [
	{ name: 'home', golden: 'home_watching', art: 'stardust-poster.png' },
	{ name: 'episodes', golden: 'episodes', art: 'stardust-wide.png' },
	{ name: 'player', golden: 'now_playing' },
	{ name: 'reading', golden: 'read_home_history' },
	{ name: 'read-search', golden: 'read_search_mid' },
];
const sizes = ['120x34', '80x24'];

const CW = 9; // cell width in px, 0.6 of the font size
const CH = 18;
const FONT = 15;
const PAD = 18;

type Style = { fg?: string; bg?: string; mods: Set<string> };
const PLAIN: Style = { mods: new Set() };

function width(ch: string): number {
	const c = ch.codePointAt(0)!;
	if (c >= 0x1100 && (c <= 0x115f || (c >= 0x2e80 && c <= 0xa4cf) || (c >= 0xac00 && c <= 0xd7a3) || (c >= 0xf900 && c <= 0xfaff) || (c >= 0xfe30 && c <= 0xfe4f) || (c >= 0xff00 && c <= 0xff60) || (c >= 0xffe0 && c <= 0xffe6) || (c >= 0x1f300 && c <= 0x1faff))) return 2;
	return 1;
}

// Block elements as [x, y, w, h] fractions of the cell; shades as an opacity over the whole cell.
const blocks: Record<string, [number, number, number, number]> = {
	'█': [0, 0, 1, 1], '▀': [0, 0, 1, 0.5], '▄': [0, 0.5, 1, 0.5], '▐': [0.5, 0, 0.5, 1], '▔': [0, 0, 1, 0.125], '▕': [0.875, 0, 0.125, 1],
	'▏': [0, 0, 0.125, 1], '▎': [0, 0, 0.25, 1], '▍': [0, 0, 0.375, 1], '▌': [0, 0, 0.5, 1], '▋': [0, 0, 0.625, 1], '▊': [0, 0, 0.75, 1], '▉': [0, 0, 0.875, 1],
	'▁': [0, 0.875, 1, 0.125], '▂': [0, 0.75, 1, 0.25], '▃': [0, 0.625, 1, 0.375], '▅': [0, 0.375, 1, 0.625], '▆': [0, 0.25, 1, 0.75], '▇': [0, 0.125, 1, 0.875],
};
const shades: Record<string, number> = { '░': 0.22, '▒': 0.45, '▓': 0.7 };
// Box drawing as segments from the cell centre: left, right, up, down.
const boxes: Record<string, string> = {
	'─': 'lr', '│': 'ud', '┌': 'rd', '┐': 'ld', '└': 'ru', '┘': 'lu', '├': 'rud', '┤': 'lud', '┬': 'lrd', '┴': 'lru', '┼': 'lrud',
	'╭': 'rd', '╮': 'ld', '╰': 'ru', '╯': 'lu',
};

function esc(s: string) {
	return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

async function draw(golden: string, size: string, artFile?: string) {
	const [cols, rows] = size.split('x').map(Number);
	const src = readFileSync(join(goldens, `${golden}@${size}.txt`), 'utf8');
	const [grid, styles] = src.split('\n=== styles ===\n');
	const colours: Record<string, string> = {};
	const styleLines = styles.trim().split('\n');
	for (const pair of styleLines[0].replace(/^colours /, '').split(' ')) {
		const [k, v] = pair.split('=');
		colours[k] = v;
	}
	const colour = (v?: string) => (v ? (colours[v] ?? v) : undefined);
	const cells: { ch: string; st: Style }[][] = [];
	const lines = grid.split('\n');
	for (let r = 0; r < rows; r++) {
		const row: { ch: string; st: Style }[] = [];
		for (const ch of lines[r] ?? '') {
			row.push({ ch, st: PLAIN });
			if (width(ch) === 2) row.push({ ch: '', st: PLAIN });
		}
		while (row.length < cols) row.push({ ch: ' ', st: PLAIN });
		cells.push(row);
	}
	for (const line of styleLines.slice(1)) {
		const m = line.match(/^r(\d+) c(\d+)-(\d+) (.*)$/);
		if (!m) continue;
		const st: Style = { mods: new Set() };
		for (const part of m[4].split(' ')) {
			const [k, v] = part.split('=');
			if (k === 'fg') st.fg = v;
			else if (k === 'bg') st.bg = v;
			else if (k === 'mod') v.split('|').forEach((x) => st.mods.add(x));
		}
		for (let c = +m[2]; c <= +m[3]; c++) cells[+m[1]][c].st = st;
	}

	const ground = colours.ground ?? '#1a1b26';
	const text = colours.text ?? '#e4e0d6';
	const W = cols * CW + PAD * 2;
	const H = rows * CH + PAD * 2;
	const body: string[] = [];
	const at = (c: number, r: number) => [PAD + c * CW, PAD + r * CH];

	// The cover plate: the largest block of wide shade runs three or more rows tall.
	let plate: { r0: number; r1: number; c0: number; c1: number } | undefined;
	if (artFile) {
		const spans = cells.map((row) => {
			const s = row.map((x) => x.ch).join('');
			// Progress tracks are eight cells or fewer; a plate is wider.
			const m = s.match(/░{12,}/);
			return m ? [m.index!, m.index! + m[0].length - 1] : undefined;
		});
		for (let r = 0; r < rows; r++) {
			const s = spans[r];
			if (!s) continue;
			let r1 = r;
			while (r1 + 1 < rows && spans[r1 + 1]?.[0] === s[0] && spans[r1 + 1]?.[1] === s[1]) r1++;
			if (r1 - r >= 2 && (!plate || (r1 - r) * (s[1] - s[0]) > (plate.r1 - plate.r0) * (plate.c1 - plate.c0))) plate = { r0: r, r1, c0: s[0], c1: s[1] };
			r = r1;
		}
	}
	const inPlate = (r: number, c: number) => plate && r >= plate.r0 && r <= plate.r1 && c >= plate.c0 && c <= plate.c1;

	for (let r = 0; r < rows; r++) {
		const row = cells[r];
		// Backgrounds.
		for (let c = 0; c < cols; ) {
			const st = row[c].st;
			const rev = st.mods.has('REVERSED');
			const bg = rev ? colour(st.fg) ?? text : colour(st.bg);
			let e = c;
			while (e + 1 < cols && row[e + 1].st === st) e++;
			if (bg) {
				const [x, y] = at(c, r);
				body.push(`<rect x="${x}" y="${y}" width="${(e - c + 1) * CW}" height="${CH}" fill="${bg}"/>`);
			}
			c = e + 1;
		}
		// Shapes and text. Blocks that touch in a row become one rect, so no seam shows between them
		// when the drawing is scaled.
		const fills: { x: number; y: number; w: number; h: number; fg: string; op: number }[] = [];
		const fill = (x: number, y: number, w: number, h: number, fg: string, op: number) => {
			const last = fills.at(-1);
			if (last && last.y === y && last.h === h && last.fg === fg && last.op === op && Math.abs(last.x + last.w - x) < 0.01) last.w += w;
			else fills.push({ x, y, w, h, fg, op });
		};
		let run = '';
		let runStart = 0;
		let runStyle: Style | undefined;
		const flush = () => {
			const trimmed = run.replace(/\s+$/, '');
			const lead = trimmed.length - trimmed.trimStart().length;
			const t = trimmed.trimStart();
			if (t && runStyle) {
				const st = runStyle;
				const rev = st.mods.has('REVERSED');
				const fg = rev ? colour(st.bg) ?? ground : colour(st.fg) ?? text;
				const n = [...t].reduce((a, ch) => a + width(ch), 0);
				const [x, y] = at(runStart + lead, r);
				const attrs = [`x="${x}"`, `y="${y + CH * 0.73}"`, `fill="${fg}"`];
				if (n > 1) attrs.push(`textLength="${n * CW}"`, 'lengthAdjust="spacing"');
				if (st.mods.has('BOLD')) attrs.push('font-weight="700"');
				if (st.mods.has('DIM')) attrs.push('opacity="0.6"');
				if (st.mods.has('ITALIC')) attrs.push('font-style="italic"');
				if (st.mods.has('UNDERLINED')) attrs.push('text-decoration="underline"');
				body.push(`<text ${attrs.join(' ')}>${esc(t)}</text>`);
			}
			run = '';
			runStyle = undefined;
		};
		for (let c = 0; c < cols; c++) {
			const { ch, st } = row[c];
			if (ch === '') continue;
			const rev = st.mods.has('REVERSED');
			const fg = rev ? colour(st.bg) ?? ground : colour(st.fg) ?? text;
			const [x, y] = at(c, r);
			const shape = blocks[ch] || shades[ch] !== undefined || boxes[ch];
			if (shape || (runStyle && runStyle !== st)) flush();
			if (inPlate(r, c)) continue;
			if (blocks[ch]) {
				const [bx, by, bw, bh] = blocks[ch];
				fill(x + bx * CW, y + by * CH, bw * CW, bh * CH, fg, 1);
			} else if (shades[ch] !== undefined) {
				fill(x, y, CW, CH, fg, shades[ch]);
			} else if (boxes[ch]) {
				const cx = x + CW / 2;
				const cy = y + CH / 2;
				const segs: string[] = [];
				for (const d of boxes[ch]) {
					if (d === 'l') segs.push(`M${x} ${cy}H${cx}`);
					if (d === 'r') segs.push(`M${cx} ${cy}H${x + CW}`);
					if (d === 'u') segs.push(`M${cx} ${y}V${cy}`);
					if (d === 'd') segs.push(`M${cx} ${cy}V${y + CH}`);
				}
				body.push(`<path d="${segs.join('')}" stroke="${fg}" stroke-width="1" fill="none"/>`);
			} else {
				if (!runStyle) {
					runStyle = st;
					runStart = c;
				}
				run += ch;
			}
		}
		flush();
		for (const f of fills) {
			const op = f.op < 1 ? ` opacity="${f.op}"` : '';
			body.push(`<rect x="${f.x}" y="${f.y}" width="${f.w}" height="${f.h}" fill="${f.fg}"${op} shape-rendering="crispEdges"/>`);
		}
	}

	if (plate && artFile) {
		const [x, y] = at(plate.c0, plate.r0);
		const w = (plate.c1 - plate.c0 + 1) * CW;
		const h = (plate.r1 - plate.r0 + 1) * CH;
		const meta = await sharp(join(art, artFile)).metadata();
		const scale = Math.min(w / meta.width!, h / meta.height!);
		const iw = Math.round(meta.width! * scale);
		const ih = Math.round(meta.height! * scale);
		const png = await sharp(join(art, artFile)).resize(iw * 2, ih * 2).webp({ quality: 82 }).toBuffer();
		body.push(`<image x="${x}" y="${y}" width="${iw}" height="${ih}" href="data:image/webp;base64,${png.toString('base64')}"/>`);
	}

	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${W} ${H}" width="${W}" height="${H}" font-family="'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', 'Liberation Mono', monospace" font-size="${FONT}" xml:space="preserve">
<rect width="${W}" height="${H}" fill="${ground}"/>
${body.join('\n')}
</svg>
`;
}

mkdirSync(out, { recursive: true });
for (const s of screens) {
	for (const size of sizes) {
		const svg = await draw(s.golden, size, s.art);
		const file = join(out, `${s.name}-${size.split('x')[0]}.svg`);
		writeFileSync(file, svg);
		console.log(file.replace(`${repo}/`, ''), `${Math.round(svg.length / 1024)} kB`);
	}
}
