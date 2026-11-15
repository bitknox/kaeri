// The receiver's strings: the JSON tools/i18n writes from i18n/<lang>/receiver-cast.ftl into
// public/cast/receiver/i18n/. A message is a list of text parts and `{ arg }` parts, or a plural.

type Parts = (string | { arg: string })[];
type Body = Parts | { plural: string; forms: Record<string, Parts> };

export interface Index {
	source: string;
	languages: string[];
	args: Record<string, Record<string, string>>;
}

export interface Pack {
	tag: string;
	messages: Record<string, Body>;
}

/** The best of `available` for a BCP 47 tag: exact, then the same language, else null. */
export function pickLanguage(available: string[], wanted: string | undefined | null): string | null {
	if (!wanted) return null;
	const want = wanted.toLowerCase();
	const exact = available.find((t) => t.toLowerCase() === want);
	if (exact) return exact;
	const base = want.split(/[-_]/)[0];
	return available.find((t) => t.toLowerCase().split('-')[0] === base) ?? null;
}

export class Strings {
	constructor(
		readonly index: Index,
		readonly pack: Pack,
		readonly source: Pack,
	) {}

	get tag() {
		return this.pack.tag;
	}

	t(id: string, args: Record<string, string | number> = {}): string {
		const body = this.pack.messages[id] ?? this.source.messages[id];
		if (!body) return id;
		const tag = this.pack.messages[id] ? this.pack.tag : this.source.tag;
		const types = this.index.args[id] ?? {};
		const parts = Array.isArray(body)
			? body
			: (body.forms[new Intl.PluralRules(tag).select(Number(args[body.plural] ?? 0))] ?? body.forms.other);
		return parts
			.map((p) => {
				if (typeof p === 'string') return p;
				const v = args[p.arg];
				if (v == null) return '';
				if (typeof v === 'number' && types[p.arg] === 'int') return new Intl.NumberFormat(tag).format(v);
				return String(v);
			})
			.join('');
	}
}

export async function loadStrings(base: string, wanted: string | null, fetcher: typeof fetch = fetch): Promise<Strings> {
	const get = async <T>(path: string): Promise<T> => {
		const r = await fetcher(new URL(path, base).href);
		if (!r.ok) throw new Error(`${path}: ${r.status}`);
		return (await r.json()) as T;
	};
	const index = await get<Index>('i18n/index.json');
	const source = await get<Pack>(`i18n/${index.source}.json`);
	const tag = pickLanguage(index.languages, wanted) ?? index.source;
	const pack = tag === index.source ? source : await get<Pack>(`i18n/${tag}.json`).catch(() => source);
	return new Strings(index, pack, source);
}
