/**
 * Tooltips drawn by the app in the theme's colours.
 *
 * A native `title` tooltip belongs to the platform: on Linux, WebKitGTK draws
 * it in the desktop's own yellow, and Windows and macOS in their own styles.
 * CSS cannot reach any of them, so, as with select menus (selectMenu.ts), the
 * popup is replaced and the attribute is not:
 *
 * - `installTooltips()` (once, in the root layout) shows every element's
 *   `title` in the app's tooltip. While the pointer is on the element its
 *   title is moved aside, so the native one does not show as well, and put
 *   back when the pointer leaves. Markup keeps using plain `title`.
 * - `use:tooltip={text}` is for a tooltip whose first line is a heading,
 *   shown bold, such as a compact session row's details.
 *
 * Mouse only (touch never showed a title), after a short pause; gone on
 * leave, click, scroll, Escape or the window losing focus. The tooltip lives
 * on <body>, so a scrolled or transformed list cannot clip it.
 */

const DELAY_MS = 450;
const GAP = 8;
/** Where a hovered element's title is kept while it is moved aside. */
const KEPT = 'data-app-title';

let tip: HTMLDivElement | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
let pointer = { x: 0, y: 0 };

function hideTip(): void {
	clearTimeout(timer);
	timer = undefined;
	tip?.remove();
	tip = null;
}

function showTip(text: string, heading: boolean): void {
	hideTip();
	if (!text.trim()) return;
	tip = document.createElement('div');
	tip.className = 'app-tooltip';
	tip.setAttribute('role', 'tooltip');
	if (heading) {
		const [head, ...rest] = text.split('\n');
		const strong = document.createElement('div');
		strong.className = 'app-tooltip-head';
		strong.textContent = head;
		tip.append(strong);
		const more = rest.join('\n').replace(/^\n+/, '');
		if (more) {
			const body = document.createElement('div');
			body.className = 'app-tooltip-body';
			body.textContent = more;
			tip.append(body);
		}
	} else {
		tip.textContent = text;
	}
	document.body.append(tip);
	// Below and right of the pointer, flipped or nudged to stay on screen.
	const r = tip.getBoundingClientRect();
	const { x, y } = pointer;
	let left = x + GAP;
	let top = y + GAP * 2;
	if (left + r.width > window.innerWidth - GAP) left = Math.max(GAP, window.innerWidth - GAP - r.width);
	if (top + r.height > window.innerHeight - GAP) top = Math.max(GAP, y - GAP - r.height);
	tip.style.left = `${left}px`;
	tip.style.top = `${top}px`;
}

function schedule(text: () => string, heading: boolean, owner: Element): void {
	hideTip();
	timer = setTimeout(() => {
		if (owner.isConnected) showTip(text(), heading);
	}, DELAY_MS);
}

/** The element whose title is moved aside right now, if any. */
let owner: HTMLElement | SVGElement | null = null;

function release(): void {
	hideTip();
	if (!owner) return;
	const kept = owner.getAttribute(KEPT);
	owner.removeAttribute(KEPT);
	// The page may have set a new title meanwhile: that one wins.
	if (kept !== null && !owner.hasAttribute('title')) owner.setAttribute('title', kept);
	owner = null;
}

/** Shows every `title` in the app's tooltip. Returns the uninstaller. */
export function installTooltips(): () => void {
	function onOver(e: PointerEvent): void {
		pointer = { x: e.clientX, y: e.clientY };
		if (e.pointerType !== 'mouse') return;
		const target = e.target instanceof Element ? e.target : null;
		const el = target?.closest<HTMLElement | SVGElement>(`[title], [${KEPT}]`) ?? null;
		if (el === owner) return;
		release();
		if (!el) return;
		const text = el.getAttribute('title') ?? '';
		if (!text) return;
		owner = el;
		el.setAttribute(KEPT, text);
		el.removeAttribute('title');
		schedule(() => el.getAttribute(KEPT) ?? '', false, el);
	}

	function onOut(e: PointerEvent): void {
		if (!owner) return;
		const to = e.relatedTarget instanceof Node ? e.relatedTarget : null;
		if (to && owner.contains(to)) return;
		release();
	}

	function onMove(e: PointerEvent): void {
		if (!tip) pointer = { x: e.clientX, y: e.clientY };
	}

	function onKey(e: KeyboardEvent): void {
		if (e.key === 'Escape') hideTip();
	}

	const opts = { capture: true, passive: true } as const;
	document.addEventListener('pointerover', onOver, opts);
	document.addEventListener('pointerout', onOut, opts);
	document.addEventListener('pointermove', onMove, opts);
	document.addEventListener('pointerdown', hideTip, opts);
	document.addEventListener('keydown', onKey, true);
	window.addEventListener('scroll', hideTip, opts);
	window.addEventListener('blur', release);
	return () => {
		release();
		document.removeEventListener('pointerover', onOver, opts);
		document.removeEventListener('pointerout', onOut, opts);
		document.removeEventListener('pointermove', onMove, opts);
		document.removeEventListener('pointerdown', hideTip, opts);
		document.removeEventListener('keydown', onKey, true);
		window.removeEventListener('scroll', hideTip, opts);
		window.removeEventListener('blur', release);
	};
}

/** A tooltip whose first line is a heading. An empty text shows nothing. */
export function tooltip(node: HTMLElement, text: string) {
	let current = text;
	let inside = false;

	function onEnter(e: PointerEvent): void {
		if (e.pointerType !== 'mouse' || !current) return;
		inside = true;
		pointer = { x: e.clientX, y: e.clientY };
		schedule(() => current, true, node);
	}

	function onLeave(): void {
		if (!inside) return;
		inside = false;
		hideTip();
	}

	node.addEventListener('pointerenter', onEnter);
	node.addEventListener('pointerleave', onLeave);

	return {
		update(next: string) {
			current = next;
			if (inside) hideTip();
		},
		destroy() {
			onLeave();
			node.removeEventListener('pointerenter', onEnter);
			node.removeEventListener('pointerleave', onLeave);
		}
	};
}
