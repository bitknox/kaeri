/**
 * The site's colours as its stylesheets write them, for the tests that check what a page paints.
 *
 * Light is `:root`. Dark is `:root` with `:root[data-theme='dark']` over it: SiteHead and the docs'
 * ThemeProvider set the attribute before the first paint. Rules inside `@media`, `@supports` and
 * `@container` are left out, since the site sets only sizes and motion there, and a rule in a
 * `@layer` loses to one outside it, as in the browser.
 */
import { readFileSync } from 'node:fs';

export type Theme = 'light' | 'dark';
export const THEMES: Theme[] = ['light', 'dark'];

export interface Rule {
	selectors: string[];
	declarations: [property: string, value: string][];
	/** Inside `@media`, `@supports` or `@container`. */
	conditional: boolean;
	/** Inside `@layer`. */
	layered: boolean;
}

/** A stylesheet in src/styles, by its file name. */
export const sheet = (name: string): string => readFileSync(new URL(name, import.meta.url), 'utf8');

/** The CSS of a component's `<style>` blocks. */
export const componentStyle = (source: string): string => [...source.matchAll(/<style\b[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join('\n');

/** Every rule of a stylesheet in source order, at-rules opened up. */
export function rules(css: string): Rule[] {
	const out: Rule[] = [];
	walk(css.replace(/\/\*[\s\S]*?\*\//g, ''), false, false, out);
	return out;
}

function walk(css: string, conditional: boolean, layered: boolean, out: Rule[]) {
	let at = 0;
	for (;;) {
		const open = css.indexOf('{', at);
		if (open < 0) return;
		const close = closing(css, open);
		// What stands before the block, after any statement such as `@import '…';`.
		const head = css.slice(at, open).split(';').at(-1)!.trim();
		const body = css.slice(open + 1, close);
		if (/^@(media|supports|container)\b/.test(head)) walk(body, true, layered, out);
		else if (/^@layer\b/.test(head)) walk(body, conditional, true, out);
		else if (!head.startsWith('@')) {
			const declarations = split(flat(body), ';').map((d): [string, string] => {
				const colon = d.indexOf(':');
				return [d.slice(0, colon).trim(), d.slice(colon + 1).trim()];
			});
			out.push({ selectors: split(head, ','), declarations, conditional, layered });
		}
		at = close + 1;
	}
}

/** A rule's body without the blocks nested in it, such as `@starting-style`. */
function flat(body: string): string {
	let out = '';
	let at = 0;
	for (;;) {
		const open = body.indexOf('{', at);
		if (open < 0) return out + body.slice(at);
		out += body.slice(at, Math.max(at, body.lastIndexOf(';', open) + 1));
		at = closing(body, open) + 1;
	}
}

/** Where the bracket opened at `open` closes, skipping quoted text. */
function closing(text: string, open: number): number {
	const pair: Record<string, string> = { '{': '}', '(': ')' };
	const shut = pair[text[open]];
	let depth = 0;
	let quote = '';
	for (let i = open; i < text.length; i++) {
		const c = text[i];
		if (quote) {
			if (c === '\\') i++;
			else if (c === quote) quote = '';
		} else if (c === '"' || c === "'") quote = c;
		else if (c === text[open]) depth++;
		else if (c === shut && --depth === 0) return i;
	}
	throw new Error(`${text.slice(open, open + 40)}… is never closed`);
}

/** Text split at a separator that is outside brackets and quotes. */
function split(text: string, separator: string): string[] {
	const parts: string[] = [];
	let depth = 0;
	let quote = '';
	let start = 0;
	for (let i = 0; i < text.length; i++) {
		const c = text[i];
		if (quote) {
			if (c === '\\') i++;
			else if (c === quote) quote = '';
		} else if (c === '"' || c === "'") quote = c;
		else if (c === '(' || c === '[') depth++;
		else if (c === ')' || c === ']') depth--;
		else if (c === separator && depth === 0) {
			parts.push(text.slice(start, i));
			start = i + 1;
		}
	}
	parts.push(text.slice(start));
	return parts.map((p) => p.trim()).filter(Boolean);
}

/** The custom properties a theme defines, from stylesheets in the order a page loads them. */
export class Palette {
	private readonly defined = new Map<string, string>();

	constructor(
		readonly theme: Theme,
		...stylesheets: string[]
	) {
		const all = stylesheets.flatMap(rules).filter((r) => !r.conditional);
		const roots = theme === 'dark' ? [':root', ":root[data-theme='dark']"] : [':root'];
		// Later wins: layered rules first, and within each, the dark rule over the plain one.
		for (const layered of [true, false])
			for (const root of roots)
				for (const r of all)
					if (r.layered === layered && r.selectors.some((s) => s.replace(/"/g, "'") === root))
						for (const [property, value] of r.declarations) if (property.startsWith('--')) this.defined.set(property, value);
	}

	/** A value with each `var()` replaced by what it comes to in this theme. */
	resolve(value: string, seen: string[] = []): string {
		const at = value.indexOf('var(');
		if (at < 0) return value.trim();
		const end = closing(value, at + 3);
		const [name, ...fallback] = split(value.slice(at + 4, end), ',');
		if (seen.includes(name)) throw new Error(`${name} refers to itself in the ${this.theme} theme`);
		const own = this.defined.get(name);
		if (own === undefined && !fallback.length) throw new Error(`${name} is not defined in the ${this.theme} theme`);
		const inner = own !== undefined ? this.resolve(own, [...seen, name]) : this.resolve(fallback.join(','), seen);
		return this.resolve(value.slice(0, at) + inner + value.slice(end + 1), seen);
	}
}

/** WCAG 2's contrast ratio between two colours written as #rgb or #rrggbb. */
export function contrast(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

function luminance(colour: string): number {
	const m = colour.trim().toLowerCase().match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/);
	if (!m) throw new Error(`${colour} is not a #rgb or #rrggbb colour`);
	const hex = m[1].length === 3 ? [...m[1]].map((c) => c + c).join('') : m[1];
	const [r, g, b] = [0, 2, 4].map((i) => {
		const c = parseInt(hex.slice(i, i + 2), 16) / 255;
		return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
	});
	return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
