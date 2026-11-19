/**
 * The setup wizard opened again from Settings.
 *
 * Not a setting: `setupComplete` stays true while it is open, so nothing that
 * waits for setup to finish (the lock screen, password prompts, the command
 * palette) goes away under a running session. In memory only; a restart
 * mid-way simply does not reopen it.
 */

export const setupWizard = $state({ rerun: false });

export function openSetupWizard(): void {
	setupWizard.rerun = true;
}

export function closeSetupWizard(): void {
	setupWizard.rerun = false;
}
