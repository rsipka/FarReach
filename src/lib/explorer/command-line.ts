/**
 * A command typed in Settings (the custom editor) as a program and its
 * arguments. Words are split on spaces; "double" or 'single' quotes keep
 * spaces in one word, so `"C:\Program Files\Editor\ed.exe" --wait` works.
 * Nothing is run through a shell.
 */
export function splitCommand(command: string): string[] {
	const words: string[] = [];
	let word = '';
	let quote: '"' | "'" | null = null;
	let started = false;
	for (const ch of command.trim()) {
		if (quote) {
			if (ch === quote) quote = null;
			else word += ch;
		} else if (ch === '"' || ch === "'") {
			quote = ch;
			started = true;
		} else if (/\s/.test(ch)) {
			if (started || word) words.push(word);
			word = '';
			started = false;
		} else {
			word += ch;
		}
	}
	if (started || word) words.push(word);
	return words;
}
