/**
 * Open a local file in the editor chosen in Settings (Terminal > Shell >
 * File editor): the system's default program, a detected editor, or a
 * custom command. A terminal editor (vim, nano...) opens in a new local
 * terminal tab.
 */
import { getSettings } from '#lib/state/settings.svelte.js';
import { createTab } from '#lib/state/tabs.svelte.js';
import { localDetectEditors, localOpen, localOpenWith, type EditorInfo } from '#lib/ipc/localfs.js';
import { splitCommand } from './command-line.js';

let detected: Promise<EditorInfo[]> | null = null;

/** The editors on this computer, looked for once per run. */
export function detectEditors(again = false): Promise<EditorInfo[]> {
	if (!detected || again) detected = localDetectEditors().catch(() => []);
	return detected;
}

function fileName(path: string): string {
	return path.split(/[\\/]/).pop() || path;
}

export async function openInEditor(path: string): Promise<void> {
	const s = getSettings();
	let program: string;
	let args: string[];
	let terminal: boolean;
	let name: string;
	if (s.fileEditor === 'custom') {
		const words = splitCommand(s.fileEditorCommand);
		if (words.length === 0) return localOpen(path);
		[program, ...args] = words;
		terminal = s.fileEditorTerminal;
		name = fileName(program);
	} else if (s.fileEditor) {
		const editor = (await detectEditors()).find((e) => e.id === s.fileEditor);
		// Uninstalled since it was picked: the system's choice rather than nothing.
		if (!editor) return localOpen(path);
		({ program, args, terminal, name } = editor);
	} else {
		return localOpen(path);
	}
	if (terminal) {
		createTab('local', `${name} · ${fileName(path)}`, undefined, undefined, undefined, { program, args: [...args, path] });
		return;
	}
	await localOpenWith(program, args, path);
}
