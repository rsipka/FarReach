/** CSS generic families, which must not be quoted: quoted, they name a font
 *  called "monospace". */
const GENERIC = new Set([
	'monospace',
	'ui-monospace',
	'serif',
	'sans-serif',
	'system-ui',
	'ui-sans-serif',
	'ui-serif',
	'cursive',
	'fantasy'
]);

/**
 * A CSS `font-family` value for the font chosen in settings: the family
 * quoted, as a name may start with a digit ("0xProto") or hold characters an
 * unquoted name cannot, then `fallback` for when it is not installed. A
 * generic family or a list already written as CSS is kept as it is.
 */
export function cssFontFamily(family: string | undefined, fallback = 'monospace'): string {
	const f = (family ?? '').trim();
	if (!f) return fallback;
	if (f.includes(',') || GENERIC.has(f.toLowerCase())) return f;
	return `"${f.replace(/["\\]/g, '\\$&')}", ${fallback}`;
}
