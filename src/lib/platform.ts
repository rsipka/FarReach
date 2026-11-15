/**
 * Which webview Reach is running in, when a fix has to be aimed at one.
 *
 * Tauri gives every platform its own engine: WebView2 (Chromium) on Windows,
 * WKWebView on macOS, WebKitGTK on Linux. Most of the app does not care. The
 * terminal does, because the two WebKits present a WebGL canvas differently
 * from Chromium (issue #47).
 */

export function isMac(): boolean {
	return typeof navigator !== 'undefined' && /Mac|iPod|iPhone|iPad/.test(navigator.platform);
}

/**
 * True inside WKWebView or WebKitGTK; false inside WebView2.
 *
 * WebView2 advertises itself as Chrome and Edge in the user agent. WebKit
 * proper does not.
 */
export function isWebKit(): boolean {
	if (typeof navigator === 'undefined') return false;
	const ua = navigator.userAgent;
	return /AppleWebKit/.test(ua) && !/Chrome|Chromium|Edg\//.test(ua);
}

/**
 * True on Windows, where a file can be dragged out without being fetched
 * first: the drop target pulls the bytes through a stream after the drop.
 * Everywhere else the file has to exist locally before the gesture begins.
 */
export function isWindows(): boolean {
	if (typeof navigator === 'undefined') return false;
	const data = (navigator as { userAgentData?: { platform?: string } }).userAgentData;
	if (data?.platform) return data.platform === 'Windows';
	return /Win/.test(navigator.platform);
}

/**
 * True on a phone or tablet (Android, iPhone, iPad). The system manages
 * windows there, and there is no hardware keyboard to count on: desktop
 * chrome goes, and the terminal and remote desktop get an extra-keys bar.
 * Android's WebView always names Android in its user agent, and an iPhone's
 * names iPhone. An iPad's does not: since iPadOS 13 its WKWebView asks for
 * desktop pages and says "Macintosh", like a Mac. It is told apart by its
 * touch screen, which no Mac has.
 */
export function isMobile(): boolean {
	if (typeof navigator === 'undefined') return false;
	const ua = navigator.userAgent;
	if (/Android|iPhone|iPad|iPod/i.test(ua)) return true;
	return /Macintosh/.test(ua) && navigator.maxTouchPoints > 1;
}
