import { defineMiddleware } from 'astro:middleware';

/**
 * Starlight writes data-theme="dark" on the docs' <html>, for its script to correct before the
 * first paint. The site is light until a visitor picks dark, so the docs start light too, and a
 * visitor without a script sees them light like every other page.
 */
export const onRequest = defineMiddleware(async (_context, next) => {
	const response = await next();
	if (!response.headers.get('content-type')?.includes('text/html')) return response;
	const html = await response.text();
	return new Response(html.replace(/(<html\b[^>]*?\sdata-theme=")dark"/, '$1light"'), response);
});
