/**
 * Release notes, as the update feed carries them (latest.json `notes`, which
 * the release workflow copies from the GitHub release's description), cut
 * into blocks the What's new dialog can draw without `{@html}`: headings,
 * list items and paragraphs, with web links picked out so they can be
 * clicked. Anything else (Markdown emphasis, tables) is shown as plain text.
 */

export type NotesPart = { text: string } | { url: string };

export interface NotesBlock {
	kind: 'heading' | 'item' | 'para';
	parts: NotesPart[];
}

/** http(s) links only, ending before trailing punctuation or a closing bracket. */
const URL_RE = /https?:\/\/[^\s<>"')\]]+[^\s<>"')\].,;:!?]/g;

/** A line's text split into plain text and links. Markdown `[label](url)`
 *  keeps only the link; `**bold**` and backticks lose their markers. */
export function splitLinks(line: string): NotesPart[] {
	const clean = line
		.replace(/\[([^\]]*)\]\((https?:\/\/[^)\s]+)\)/g, (_m, label: string, url: string) =>
			label && label !== url ? `${label} (${url})` : url
		)
		.replace(/\*\*([^*]+)\*\*/g, '$1')
		.replace(/`([^`]+)`/g, '$1');
	const parts: NotesPart[] = [];
	let last = 0;
	for (const m of clean.matchAll(URL_RE)) {
		const at = m.index ?? 0;
		if (at > last) parts.push({ text: clean.slice(last, at) });
		parts.push({ url: m[0] });
		last = at + m[0].length;
	}
	if (last < clean.length) parts.push({ text: clean.slice(last) });
	return parts;
}

/** The notes as blocks. Lines of one paragraph are joined with a space;
 *  an empty line, a heading or a list item ends it. */
export function parseReleaseNotes(notes: string | null | undefined): NotesBlock[] {
	if (!notes) return [];
	const blocks: NotesBlock[] = [];
	let para: string[] = [];
	const flush = () => {
		if (para.length) blocks.push({ kind: 'para', parts: splitLinks(para.join(' ')) });
		para = [];
	};
	for (const raw of notes.replace(/\r\n?/g, '\n').split('\n')) {
		const line = raw.trim();
		if (!line) {
			flush();
			continue;
		}
		const heading = /^#{1,6}\s+(.*)$/.exec(line);
		const item = /^(?:[-*+]|\d+[.)])\s+(.*)$/.exec(line);
		if (heading) {
			flush();
			blocks.push({ kind: 'heading', parts: splitLinks(heading[1]) });
		} else if (item) {
			flush();
			blocks.push({ kind: 'item', parts: splitLinks(item[1]) });
		} else {
			para.push(line);
		}
	}
	flush();
	return blocks;
}
