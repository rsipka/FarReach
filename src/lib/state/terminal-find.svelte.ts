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

/** Asks for the find field of the tab's bar (Ctrl+Shift+F from the terminal). */
let findRequest = $state<{ tabId: string; at: number } | null>(null);

export function requestFind(tabId: string): void {
	findRequest = { tabId, at: Date.now() };
}

export function pendingFind(): { tabId: string; at: number } | null {
	return findRequest;
}
