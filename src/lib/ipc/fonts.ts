import { invoke } from '@tauri-apps/api/core';

/** One font family installed on this computer. */
export interface InstalledFont {
	name: string;
	/** Any face of the family is marked fixed pitch. */
	monospace: boolean;
}

/** The font families installed on this computer, sorted by name. Empty on
 *  the phones, which have no font folders to read. */
export async function fontsListInstalled(): Promise<InstalledFont[]> {
	return invoke<InstalledFont[]>('fonts_list_installed');
}
