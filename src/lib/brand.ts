/**
 * Reacher CM fork branding, kept in one place.
 *
 * Reacher CM is a fork of Reach (https://github.com/alexandrosnt/Reach). The
 * locale files stay word for word as upstream's, so merging upstream stays
 * easy; the product name in them is swapped when a locale loads instead.
 */

export const APP_NAME = 'Reacher CM';
export const REPO_URL = 'https://github.com/rsipka/ReacherCM';
export const UPSTREAM_URL = 'https://github.com/alexandrosnt/Reach';

/**
 * Whether to show the "Join the community" parts: the launch prompt and the
 * Discord row in Settings > General. The Discord is upstream's, and Reacher CM
 * has no community of its own yet, so they are hidden. Set this to true to
 * bring them all back; nothing else was removed.
 */
export const SHOW_COMMUNITY = false;

/**
 * Whether to show the "Support Reach" row in Settings > General. It links to
 * the original author's GitHub Sponsors, so it is hidden for now. Set this to
 * true to bring it back; nothing else was removed.
 */
export const SHOW_SPONSOR = false;

/**
 * Keys where "Reach" is not the product name, or where it means the
 * original project (its community and sponsors), so they are left alone.
 */
const KEEP: ReadonlySet<string> = new Set([
	'connect.step_connect', // the verb, "Reach"
	'k8s.reach_through', // the verb, "Reach the cluster through"
	'settings.support', // sponsors the original author
	'settings.support_desc',
	'community.title', // the Discord is upstream's
	'community.body'
]);

const NAME = /\bReach(?=s\b|\b)/g;

/** A locale string with the product name swapped for {@link APP_NAME}. */
export function brand(key: string, value: string): string {
	return KEEP.has(key) ? value : value.replace(NAME, APP_NAME);
}
