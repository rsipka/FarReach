/**
 * Typing into several terminals at once: the rules, kept apart from the
 * state so they can be tested as they run.
 *
 * The user chooses terminals (local or SSH tabs). What is typed into one of
 * the chosen goes to every other chosen one too; what is typed into any
 * other terminal stays in it. So a terminal left out is always safe to type
 * in, whatever is chosen elsewhere.
 */

/** The chosen terminals still open, in the order they were chosen. */
export function live(chosen: readonly string[], open: readonly string[]): string[] {
	const isOpen = new Set(open);
	return chosen.filter((id) => isOpen.has(id));
}

/** Broadcasting needs two chosen terminals: one alone has nobody to send to. */
export function isOn(chosen: readonly string[], open: readonly string[]): boolean {
	return live(chosen, open).length >= 2;
}

/** Where input typed into `source` also goes: the other chosen terminals,
 *  and only when `source` is chosen itself. */
export function recipients(chosen: readonly string[], source: string, open: readonly string[]): string[] {
	const ids = live(chosen, open);
	if (!ids.includes(source)) return [];
	return ids.filter((id) => id !== source);
}

/** Add a terminal to the chosen, or take it out. */
export function toggle(chosen: readonly string[], id: string): string[] {
	return chosen.includes(id) ? chosen.filter((x) => x !== id) : [...chosen, id];
}

/** Add terminals to the chosen, leaving those already there where they are. */
export function add(chosen: readonly string[], ids: readonly string[]): string[] {
	const out = [...chosen];
	for (const id of ids) if (!out.includes(id)) out.push(id);
	return out;
}

/**
 * How long after a key, a text input or a composition ending the terminal's
 * output to its program still counts as typed. xterm hands on everything a
 * terminal sends its program through one channel: keys and pastes, but also
 * mouse reports, the wheel turned into arrow keys, and its answers to a
 * program's questions (where is the cursor, what terminal is this). Those
 * belong to the one terminal they happened in and must not go to the others.
 */
export const TYPED_WINDOW_MS = 60;

/**
 * An answer a terminal gives its program unasked by the user: a cursor
 * position, device attributes, focus in or out, a mode, colour or window
 * report, or a mouse report. Sent to another terminal it would be read as
 * typed. `\x1b[1;5R` is also Ctrl+F3 on xterm's keyboard, so a cursor report
 * is only taken as one away from the first row's first columns with a
 * modifier; a real one there that soon after a key is rare enough.
 */
export function isTerminalReply(data: string): boolean {
	if (/^\x1b\[1;[2-8]R$/.test(data)) return false; // F3 with modifiers
	return (
		/^\x1b\[\??\d+;\d+R$/.test(data) || // cursor position
		/^\x1b\[[?>=][\d;]*c$/.test(data) || // device attributes
		/^\x1b\[[IO]$/.test(data) || // focus in / out
		/^\x1b\[\d+n$/.test(data) || // status
		/^\x1b\[\??[\d;]*\$y$/.test(data) || // mode report
		/^\x1b\[\?\d*u$/.test(data) || // keyboard flags
		/^\x1b\[\d+(;\d+)*t$/.test(data) || // window report
		/^\x1b\][\s\S]*(\x07|\x1b\\)$/.test(data) || // OSC answer (colours)
		/^\x1bP[\s\S]*\x1b\\$/.test(data) || // DCS answer
		/^\x1b\[<\d+;\d+;\d+[Mm]$/.test(data) || // SGR mouse
		/^\x1b\[M[\s\S]{3}$/.test(data) // X10 mouse
	);
}

/** Radio waves (24×24, stroked): the mark of a terminal in a broadcast. */
export const BROADCAST_ICON =
	'M12 12h.01M8.5 15.5a5 5 0 0 1 0-7M15.5 8.5a5 5 0 0 1 0 7M5.6 18.4a9 9 0 0 1 0-12.8M18.4 5.6a9 9 0 0 1 0 12.8';
