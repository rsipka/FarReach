/**
 * Smoother mouse selection on Linux.
 *
 * On Linux, xterm.js copies the selection into X11's primary selection (so a
 * middle click elsewhere pastes it) on every mouse move of a drag: it builds
 * the selected text twice, puts it in its hidden textarea, focuses it and
 * selects it. Each of those moves makes WebKitGTK lay the textarea out again
 * with the whole selection in it and claim the primary selection from the
 * desktop again, so a long selection drags behind the mouse.
 *
 * Here that copy waits until the button is let go: the highlight still follows
 * the mouse every frame, and the primary selection gets the finished text
 * once. A middle click always comes after the button is up, so it pastes the
 * same text as before.
 *
 * xterm's own option for this does not exist, so its selection service's
 * `refresh(isLinuxMouseSelection)` is wrapped. An xterm upgrade that renames
 * it leaves xterm's behaviour as it was, with a warning.
 */

import type { Terminal } from '@xterm/xterm';

interface SelectionServiceLike {
	refresh(isLinuxMouseSelection?: boolean): void;
}

/** The parts of the DOM this needs, so it can be tested without a browser. */
export interface SelectionSyncDeps {
	/** Where a press starts a selection (the terminal element). */
	host: Pick<EventTarget, 'addEventListener' | 'removeEventListener'>;
	/** Where the release is heard, even outside the terminal (window). */
	win: Pick<EventTarget, 'addEventListener' | 'removeEventListener'>;
	/** Runs after xterm's own mouseup handler. */
	later?: (fn: () => void) => void;
	warn?: (message: string) => void;
}

export function deferLinuxSelectionSync(term: Terminal, deps: SelectionSyncDeps): () => void {
	const { host, win, later = (fn) => setTimeout(fn), warn = (m) => console.warn(m) } = deps;
	const service = (term as unknown as { _core?: { _selectionService?: SelectionServiceLike } })._core
		?._selectionService;
	if (!service || typeof service.refresh !== 'function') {
		warn('[Terminal] Selection sync not deferred: unrecognised xterm internals');
		return () => {};
	}

	const original = service.refresh;
	let dragging = false;
	let pending = false;

	function wrapped(this: SelectionServiceLike, isLinuxMouseSelection?: boolean): void {
		if (isLinuxMouseSelection && dragging) {
			pending = true;
			original.call(this, false);
			return;
		}
		original.call(this, isLinuxMouseSelection);
	}
	service.refresh = wrapped;

	function onDown(e: Event): void {
		if ((e as MouseEvent).button !== 0) return;
		dragging = true;
		pending = false;
	}
	function onUp(e: Event): void {
		if ((e as MouseEvent).button !== 0 || !dragging) return;
		dragging = false;
		if (!pending) return;
		pending = false;
		later(() => original.call(service, true));
	}
	host.addEventListener('mousedown', onDown, true);
	win.addEventListener('mouseup', onUp, true);

	return () => {
		host.removeEventListener('mousedown', onDown, true);
		win.removeEventListener('mouseup', onUp, true);
		if (service.refresh === wrapped) service.refresh = original;
	};
}
