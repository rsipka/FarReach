/**
 * OSC 52: a program on the server puts text on this computer's clipboard
 * (tmux copy mode, vim, neovim), as `ESC ] 52 ; <selection> ; <base64> BEL`.
 *
 * Only writing is supported: a query (`?`) for the clipboard's contents is
 * never answered, so a server cannot read what you copied elsewhere. Off
 * unless Settings say so.
 */

/** Longest base64 accepted: about 750 KB of text. */
export const MAX_OSC52_BASE64 = 1_000_000;

/**
 * The text an OSC 52 sequence's data (what follows `52;`) asks to copy, or
 * null when it asks for nothing this app does: a query, a clear, too
 * much, or not base64.
 */
export function osc52Text(data: string): string | null {
	const semi = data.indexOf(';');
	if (semi < 0) return null;
	const b64 = data.slice(semi + 1).trim();
	if (!b64 || b64 === '?' || b64.length > MAX_OSC52_BASE64) return null;
	if (!/^[A-Za-z0-9+/]*={0,2}$/.test(b64)) return null;
	try {
		const bin = atob(b64);
		const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
		const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		return text || null;
	} catch {
		return null;
	}
}
