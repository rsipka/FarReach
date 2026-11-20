/**
 * The app fitted above an iPhone's or iPad's on-screen keyboard.
 *
 * iOS does not make the web view smaller for the keyboard. It lays it over
 * the page and scrolls the whole page up until the focused field shows, so
 * the title bar and tabs slid off the top, half the app was gone and it all
 * came back down when the keyboard closed. `100dvh` does not follow the
 * keyboard on iOS either.
 *
 * The part of the page still visible is the visual viewport. The shell is
 * pinned to it (--vv-top, --vv-height, read by AppShell), so the title bar
 * and tabs stay put, the terminal gets shorter and refits, and its cursor
 * line sits just above the keyboard (or the extra-keys bar). While the
 * keyboard is up, `keyboard-open` is set on <html>: the home indicator's
 * strip is under the keyboard then, so its padding goes.
 *
 * Android makes room for the keyboard itself (MainActivity), so this runs on
 * iOS only.
 */

const KEYBOARD_MIN = 120;

export function isIOS(): boolean {
	if (typeof navigator === 'undefined') return false;
	const ua = navigator.userAgent;
	if (/iPhone|iPad|iPod/.test(ua)) return true;
	// iPadOS asks for desktop pages and says Macintosh; no Mac has a touch screen.
	return /Macintosh/.test(ua) && navigator.maxTouchPoints > 1;
}

export function initKeyboardViewport(): () => void {
	const vv = window.visualViewport;
	if (!isIOS() || !vv) return () => {};
	const root = document.documentElement;
	root.classList.add('ios');
	let frame = 0;

	const apply = (): void => {
		frame = 0;
		// Zoom is turned off (app.html), so the scale stays 1 and the visual
		// viewport's size is the visible part of the page in CSS pixels.
		const height = Math.round(vv.height);
		const top = Math.max(0, Math.round(vv.offsetTop));
		root.style.setProperty('--vv-height', `${height}px`);
		root.style.setProperty('--vv-top', `${top}px`);
		root.classList.toggle('keyboard-open', window.innerHeight - height > KEYBOARD_MIN);
	};
	const schedule = (): void => {
		if (!frame) frame = requestAnimationFrame(apply);
	};

	apply();
	vv.addEventListener('resize', schedule);
	vv.addEventListener('scroll', schedule);
	// Pinch zoom is off for the page, but a pinch that iOS still starts as a
	// zoom (it has done so despite the viewport tag) is stopped here too.
	const noZoom = (e: Event): void => e.preventDefault();
	document.addEventListener('gesturestart', noZoom);
	document.addEventListener('gesturechange', noZoom);
	return () => {
		if (frame) cancelAnimationFrame(frame);
		vv.removeEventListener('resize', schedule);
		vv.removeEventListener('scroll', schedule);
		document.removeEventListener('gesturestart', noZoom);
		document.removeEventListener('gesturechange', noZoom);
	};
}
