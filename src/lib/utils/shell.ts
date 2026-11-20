/**
 * Quote a string as one POSIX shell word: wrap it in single quotes and write
 * each embedded single quote as '\''. Safe for spaces, $, `, ;, and newlines.
 * Idea from ReachQ (MIT, CPTProgrammer), which wrapped paths in quotes.
 */
export function shellQuote(s: string): string {
	return `'${s.replace(/'/g, `'\\''`)}'`;
}
