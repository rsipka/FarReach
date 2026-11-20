/**
 * Svelte action: a long press with a finger opens the element's right-click
 * menu, so phones and tablets reach the same menu a mouse does.
 *
 * Holding still for `LONG_PRESS_MS` dispatches a `contextmenu` event at the
 * finger, which the element's own `oncontextmenu` handler opens the menu for.
 * Moving the finger (a scroll) or lifting it early cancels. After the menu
 * opens, the tap that lifting the finger makes is swallowed, so a session
 * does not also connect, and the WebView's own long-press `contextmenu`
 * (Android fires one, iOS does not) is swallowed too, as it would land on
 * the menu's backdrop and close the menu again.
 *
 * Mouse and pen are left alone: they have a real right click.
 *
 * Usage:
 *   <div oncontextmenu={open} use:longPress>…</div>
 */

export const LONG_PRESS_MS = 500;
const MOVE_TOLERANCE = 10;
/** How long after the finger lifts a stray click or menu event is dropped. */
const SWALLOW_MS = 400;

export function longPress(node: HTMLElement, enabled: boolean = true) {
	let on = enabled;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let startX = 0;
	let startY = 0;
	let pointerId = -1;
	let fired = false;
	let swallowUntil = 0;

	function swallowing(): boolean {
		return fired || performance.now() < swallowUntil;
	}

	function onWindowClick(e: MouseEvent): void {
		if (!swallowing()) return;
		e.preventDefault();
		e.stopPropagation();
		e.stopImmediatePropagation();
		swallowUntil = 0;
	}

	function onWindowMenu(e: MouseEvent): void {
		// Only the WebView's own event: ours is not trusted.
		if (!e.isTrusted || !swallowing()) return;
		e.preventDefault();
		e.stopPropagation();
		e.stopImmediatePropagation();
	}

	// Only while a press has opened a menu: a list can hold hundreds of rows.
	function listenSwallow(): void {
		window.addEventListener('click', onWindowClick, true);
		window.addEventListener('contextmenu', onWindowMenu, true);
	}

	function unlistenSwallow(): void {
		window.removeEventListener('click', onWindowClick, true);
		window.removeEventListener('contextmenu', onWindowMenu, true);
	}

	function cancel(): void {
		if (timer !== undefined) clearTimeout(timer);
		timer = undefined;
	}

	function finish(): void {
		cancel();
		pointerId = -1;
		if (fired) {
			swallowUntil = performance.now() + SWALLOW_MS;
			setTimeout(() => {
				if (!fired) unlistenSwallow();
			}, SWALLOW_MS + 50);
		}
		fired = false;
		window.removeEventListener('pointermove', onMove, true);
		window.removeEventListener('pointerup', finish, true);
		window.removeEventListener('pointercancel', finish, true);
	}

	function onMove(e: PointerEvent): void {
		if (e.pointerId !== pointerId || timer === undefined) return;
		if (Math.hypot(e.clientX - startX, e.clientY - startY) > MOVE_TOLERANCE) cancel();
	}

	function onDown(e: PointerEvent): void {
		if (!on || e.pointerType !== 'touch' || !e.isPrimary) return;
		finish();
		fired = false;
		pointerId = e.pointerId;
		startX = e.clientX;
		startY = e.clientY;
		const target = e.target instanceof Element ? e.target : node;
		window.addEventListener('pointermove', onMove, true);
		window.addEventListener('pointerup', finish, true);
		window.addEventListener('pointercancel', finish, true);
		timer = setTimeout(() => {
			timer = undefined;
			if (!target.isConnected) return;
			fired = true;
			listenSwallow();
			try {
				navigator.vibrate?.(15);
			} catch {
				// No vibration on this device or no permission: fine.
			}
			target.dispatchEvent(
				new MouseEvent('contextmenu', {
					bubbles: true,
					cancelable: true,
					clientX: startX,
					clientY: startY
				})
			);
		}, LONG_PRESS_MS);
	}

	node.addEventListener('pointerdown', onDown);

	return {
		update(next: boolean = true) {
			on = next;
			if (!on) finish();
		},
		destroy() {
			finish();
			node.removeEventListener('pointerdown', onDown);
			unlistenSwallow();
		}
	};
}
