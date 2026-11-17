import { invoke } from '@tauri-apps/api/core';

/** The registries whose address can be changed in Settings > Plugins. The
 *  plugin marketplace has its own field in its panel. */
export type RegistryKind = 'themes' | 'recipes';

export interface RegistryUrl {
	/** The address in effect. */
	url: string;
	/** The address the app ships with. */
	defaultUrl: string;
	isDefault: boolean;
}

export async function registryUrlGet(kind: RegistryKind): Promise<RegistryUrl> {
	return invoke<RegistryUrl>('registry_url_get', { kind });
}

/** Saves a different https address in the settings vault (needs it unlocked). */
export async function registryUrlSet(kind: RegistryKind, url: string): Promise<void> {
	return invoke('registry_url_set', { kind, url });
}

/** Back to the default address. */
export async function registryUrlReset(kind: RegistryKind): Promise<void> {
	return invoke('registry_url_reset', { kind });
}
