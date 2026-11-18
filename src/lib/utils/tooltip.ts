/**
 * A tooltip drawn by the app in the theme's colours, for text too long or too
 * important for the native `title` one, which Linux (WebKitGTK) shows in the
 * desktop's own yellow and other systems in their own style.
 *
 * `use:tooltip={text}`: shows after a short pause under a mouse, never for
 * touch, and goes on leave, click, scroll or Escape. The first line is shown
 * bold, as a heading. It lives on <body> so a scrolled or transformed list
 * cannot clip it. An empty text shows nothing.
 */

const DELAY_MS = 450;
const GAP = 8;

export function tooltip(node: HTMLElement, text: string) {
	let current = text;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let tip: HTMLDivElement | null = null;
	let x = 0;
	let y = 0;

	function hide(): void {
		clearTimeout(timer);
		timer = undefined;
		tip?.remove();
		tip = null;
		window.removeEventListener('scroll', hide, true);
		window.removeEventListener('keydown', onKey, true);
	}

	function onKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') hide();
	}

	function show(): void {
		if (!current || tip) return;
		tip = document.createElement('div');
		tip.className = 'app-tooltip';
		tip.setAttribute('role', 'tooltip');
		const [head, ...rest] = current.split('\n');
		const strong = document.createElement('div');
		strong.className = 'app-tooltip-head';
		strong.textContent = head;
		tip.append(strong);
		if (rest.length > 0) {
			const body = document.createElement('div');
			body.className = 'app-tooltip-body';
			body.textContent = rest.join('\n').replace(/^\n+/, '');
			tip.append(body);
		}
		document.body.append(tip);
		// Below and right of the pointer, flipped or nudged to stay on screen.
		const r = tip.getBoundingClientRect();
		let left = x + GAP;
		let top = y + GAP * 2;
		if (left + r.width > window.innerWidth - GAP) left = Math.max(GAP, window.innerWidth - GAP - r.width);
		if (top + r.height > window.innerHeight - GAP) top = Math.max(GAP, y - GAP - r.height);
		tip.style.left = `${left}px`;
		tip.style.top = `${top}px`;
		window.addEventListener('scroll', hide, true);
		window.addEventListener('keydown', onKey, true);
	}

	function onEnter(e: PointerEvent): void {
		if (e.pointerType !== 'mouse' || !current) return;
		x = e.clientX;
		y = e.clientY;
		clearTimeout(timer);
		timer = setTimeout(show, DELAY_MS);
	}

	function onMove(e: PointerEvent): void {
		if (tip) return;
		x = e.clientX;
		y = e.clientY;
	}

	node.addEventListener('pointerenter', onEnter);
	node.addEventListener('pointermove', onMove);
	node.addEventListener('pointerleave', hide);
	node.addEventListener('pointerdown', hide);

	return {
		update(next: string) {
			current = next;
			if (!current) hide();
			else if (tip) {
				hide();
			}
		},
		destroy() {
			hide();
			node.removeEventListener('pointerenter', onEnter);
			node.removeEventListener('pointermove', onMove);
			node.removeEventListener('pointerleave', hide);
			node.removeEventListener('pointerdown', hide);
		}
	};
}
