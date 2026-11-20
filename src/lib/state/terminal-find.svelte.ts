/**
 * Find in a terminal's scrollback, from outside it: the terminal bar's search
 * field asks the terminal of its tab through this registry, so the bar never
 * reaches into xterm itself. A terminal registers when it mounts and leaves
 * when it is torn down.
 */

export interface TerminalFinder {
	/** Next match after the selection; false when there is none. */
	next(query: string, caseSensitive: boolean): boolean;
	previous(query: string, caseSensitive: boolean): boolean;
	/** Remove the highlights and give the keyboard back to the terminal. */
	clear(): void;
}

const finders = new Map<string, TerminalFinder>();

export function registerFinder(tabId: string, finder: TerminalFinder): () => void {
	finders.set(tabId, finder);
	return () => {
		if (finders.get(tabId) === finder) finders.delete(tabId);
	};
}

export function finderFor(tabId: string): TerminalFinder | undefined {
	return finders.get(tabId);
}

export interface FindRequest {
	tabId: string;
	at: number;
	/** What to look for, from the selection; none keeps the last search. */
	text?: string;
}

/** Asks for the find field of the tab's bar (Ctrl+Shift+F from the terminal,
 *  or Find in its right-click menu). */
let findRequest = $state<FindRequest | null>(null);

export function requestFind(tabId: string, text?: string): void {
	findRequest = { tabId, at: Date.now(), text };
}

export function pendingFind(): FindRequest | null {
	return findRequest;
}

/** A request is for a bar that is there now, or mounts for it in a moment
 *  (the bar is hidden in Settings); an older one is not replayed. */
export function freshFind(r: FindRequest | null, now = Date.now()): boolean {
	return r !== null && now - r.at < 2000;
}
