/**
 * How many lines a terminal keeps above its screen (Settings > Terminal).
 *
 * xterm.js holds its scrollback in a fixed-size ring that it allocates as it
 * fills, so it needs a number: about 12 bytes per cell, roughly 1.5 MB per
 * thousand lines of a 120-column terminal. "Unlimited" is therefore a large
 * cap rather than none: 250,000 lines on a computer, 50,000 on a phone or
 * tablet, where a tab that fills it would otherwise run out of memory.
 */

/** The stored value that means "keep as much as the device allows". */
export const UNLIMITED_SCROLLBACK = 0;

/** What Settings offers, in order; 0 is Unlimited. */
export const SCROLLBACK_CHOICES = [1000, 10000, 50000, 100000, UNLIMITED_SCROLLBACK] as const;

export const DEFAULT_SCROLLBACK = UNLIMITED_SCROLLBACK;

/** Unlimited, in lines. */
export const UNLIMITED_LINES = 250_000;
export const UNLIMITED_LINES_MOBILE = 50_000;

/** A stored setting, made safe: a whole number of lines, or 0 for Unlimited. */
export function scrollbackSetting(value: unknown): number {
	if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) return DEFAULT_SCROLLBACK;
	const lines = Math.floor(value);
	return lines > UNLIMITED_LINES ? UNLIMITED_LINES : lines;
}

/** The number xterm.js is given for a setting, on this kind of device. */
export function scrollbackLines(setting: unknown, mobile: boolean): number {
	const cap = mobile ? UNLIMITED_LINES_MOBILE : UNLIMITED_LINES;
	const lines = scrollbackSetting(setting);
	return lines === UNLIMITED_SCROLLBACK ? cap : Math.min(lines, cap);
}
