/**
 * The terminal bell (BEL, ^G): what happens when a program in a terminal
 * rings it. Each of the three ways is a setting of its own, with defaults
 * per platform (see #lib/state/settings-store):
 *
 * - sound: a short ping, unless Settings > General > Play sounds is off;
 * - flash: the terminal flashes briefly, where it can be seen;
 * - notification: a system notification, only when the tab is not in view
 *   or the app is in the background, so a bell in a long job can call you
 *   back to it.
 *
 * A program can ring hundreds of times a second (`yes $'\a'`, a held key at
 * a full prompt), so each way has its own pace.
 */

import { invoke } from '@tauri-apps/api/core';
import { isMac, isMobile, isWebKit } from '#lib/platform.js';

const VOLUME = 0.3;
/** At most one sound and one flash per this many ms. */
export const SOUND_GAP_MS = 150;
/** At most one notification per tab per this many ms. */
export const NOTIFY_GAP_MS = 10_000;

export interface BellWays {
	sound: boolean;
	flash: boolean;
	notify: boolean;
}

export interface BellContext {
	/** The tab is on screen and the app's window has focus. */
	seen: boolean;
	/** When this tab last notified, in ms (0: never). */
	lastNotified: number;
	/** When any bell last made a sound, in ms (0: never). */
	lastSound: number;
	now: number;
	/** Settings > General > Play sounds. */
	soundsOn: boolean;
}

/** What one ring does, given the settings and where the tab is. */
export function bellActions(ways: BellWays, ctx: BellContext): BellWays {
	return {
		sound: ways.sound && ctx.soundsOn && ctx.now - ctx.lastSound >= SOUND_GAP_MS,
		flash: ways.flash && ctx.seen,
		notify: ways.notify && !ctx.seen && ctx.now - ctx.lastNotified >= NOTIFY_GAP_MS
	};
}

let lastSound = 0;

/** The bell's sound. On Linux, Rust plays it, for the reason in #lib/state/toasts. */
export function playBell(now = Date.now()): void {
	lastSound = now;
	if (isWebKit() && !isMac() && !isMobile()) {
		invoke('play_bell_sound', { volume: VOLUME }).catch(() => {});
		return;
	}
	try {
		const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
		if (!Ctx) return;
		const ctx = new Ctx();
		const gain = ctx.createGain();
		gain.connect(ctx.destination);
		const t = ctx.currentTime;
		gain.gain.setValueAtTime(0.0001, t);
		gain.gain.exponentialRampToValueAtTime(VOLUME, t + 0.004);
		gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.15);
		for (const [freq, level] of [[880, 0.7], [1760, 0.3]] as const) {
			const osc = ctx.createOscillator();
			const g = ctx.createGain();
			osc.frequency.value = freq;
			g.gain.value = level;
			osc.connect(g).connect(gain);
			osc.start(t);
			osc.stop(t + 0.16);
		}
		setTimeout(() => void ctx.close().catch(() => {}), 400);
	} catch {
		// No audio here
	}
}

export function lastBellSound(): number {
	return lastSound;
}

/** The app is in front and this page is shown. */
export function windowSeen(): boolean {
	return typeof document !== 'undefined' && document.visibilityState === 'visible' && document.hasFocus();
}

/** Ask the system for permission to notify, if it has not been given yet.
 *  True when notifications may be sent. */
export async function allowNotifications(): Promise<boolean> {
	try {
		const n = await import('@tauri-apps/plugin-notification');
		if (await n.isPermissionGranted()) return true;
		return (await n.requestPermission()) === 'granted';
	} catch {
		return false;
	}
}

/** A system notification for a bell in `name`. */
export async function notifyBell(title: string, body: string): Promise<void> {
	try {
		const n = await import('@tauri-apps/plugin-notification');
		if (!(await n.isPermissionGranted())) return;
		n.sendNotification({ title, body });
	} catch {
		// Notifications not available
	}
}
