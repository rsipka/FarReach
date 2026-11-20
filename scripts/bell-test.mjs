/**
 * What one ring of the terminal bell does (#lib/terminal/bell).
 * Run: node --import ./scripts/resolve-lib.mjs scripts/bell-test.mjs
 */
import { bellActions, SOUND_GAP_MS, NOTIFY_GAP_MS } from '#lib/terminal/bell.js';

let failures = 0;
const check = (name, cond) => {
	console.log(`  ${cond ? 'PASS' : 'FAIL'}  ${name}`);
	if (!cond) failures++;
};
const all = { sound: true, flash: true, notify: true };
const ctx = (o = {}) => ({ seen: true, lastNotified: 0, lastSound: 0, now: 1_000_000, soundsOn: true, ...o });
const eq = (a, b) => JSON.stringify(a) === JSON.stringify(b);

check('seen tab: sound and flash, no notification', eq(bellActions(all, ctx()), { sound: true, flash: true, notify: false }));
check('out of view: sound and notification, no flash', eq(bellActions(all, ctx({ seen: false })), { sound: true, flash: false, notify: true }));
check('Play sounds off silences the bell', !bellActions(all, ctx({ soundsOn: false })).sound);
check('nothing on: nothing happens', eq(bellActions({ sound: false, flash: false, notify: false }, ctx({ seen: false })), { sound: false, flash: false, notify: false }));
check('a ring right after a sound is silent', !bellActions(all, ctx({ lastSound: 1_000_000 - SOUND_GAP_MS + 1 })).sound);
check('a ring after the gap sounds again', bellActions(all, ctx({ lastSound: 1_000_000 - SOUND_GAP_MS })).sound);
check('one notification per tab per gap', !bellActions(all, ctx({ seen: false, lastNotified: 1_000_000 - NOTIFY_GAP_MS + 1 })).notify);
check('notifies again after the gap', bellActions(all, ctx({ seen: false, lastNotified: 1_000_000 - NOTIFY_GAP_MS })).notify);

console.log(failures ? `\n${failures} failed` : '\nall passed');
process.exit(failures ? 1 : 0);
