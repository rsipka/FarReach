/**
 * The apps sessions can be imported from, as the import dialog sees them:
 * what files to offer, how to read one, and whether the app's own settings
 * can be read from this computer.
 */

import { invoke } from '@tauri-apps/api/core';
import { isWindows } from '#lib/platform.js';
import { parseRdmExport } from '../rdm-import.ts';
import { decodeFile, type ImportParseResult } from './common.ts';
import { parseMobaXterm } from './mobaxterm.ts';
import { parseMremoteNg } from './mremoteng.ts';
import { parsePutty } from './putty.ts';
import { parseTabby } from './tabby.ts';
import { parseWinScp } from './winscp.ts';

export type ImportSourceId = 'rdm' | 'mremoteng' | 'mobaxterm' | 'putty' | 'winscp' | 'tabby';

export interface ImportSource {
	id: ImportSourceId;
	/** The app's name, as shown. */
	app: string;
	/** File types the picker offers. */
	accept: string;
	/** Tag put on every imported session. */
	tag: string;
	/** Where `import_read_local` can find the app's settings, if anywhere on this platform. */
	local: boolean;
	placeholder: string;
	/** `password`: undefined when none was asked for yet, null to go on without passwords. */
	parse(raw: string, password?: string | null): Promise<ImportParseResult>;
}

/** Tabby's config is YAML; the backend turns it into JSON. */
async function tabbyData(raw: string): Promise<unknown> {
	const trimmed = raw.trim();
	if (trimmed.startsWith('{')) return JSON.parse(trimmed);
	return invoke<unknown>('import_yaml_to_json', { text: raw });
}

export const IMPORT_SOURCES: Record<ImportSourceId, ImportSource> = {
	rdm: {
		id: 'rdm',
		app: 'Remote Desktop Manager',
		accept: '.rdm,.json,.xml',
		tag: 'rdm-import',
		local: false,
		placeholder: '<?xml version="1.0"?>\n<ArrayOfConnection>\n  <Connection>…</Connection>\n</ArrayOfConnection>',
		parse: async (raw) => parseRdmExport(raw)
	},
	mremoteng: {
		id: 'mremoteng',
		app: 'mRemoteNG',
		accept: '.xml',
		tag: 'mremoteng-import',
		local: isWindows(),
		placeholder: '<?xml version="1.0" encoding="utf-8"?>\n<mrng:Connections …>\n  <Node Name="…" Type="Connection" … />\n</mrng:Connections>',
		parse: parseMremoteNg
	},
	mobaxterm: {
		id: 'mobaxterm',
		app: 'MobaXterm',
		accept: '.mxtsessions,.ini',
		tag: 'mobaxterm-import',
		local: isWindows(),
		placeholder: '[Bookmarks]\nSubRep=\nImgNum=42\nweb1=#109#0%web1.example.com%22%deploy%…',
		parse: parseMobaXterm
	},
	putty: {
		id: 'putty',
		app: 'PuTTY',
		accept: '.reg',
		tag: 'putty-import',
		local: true,
		placeholder: 'Windows Registry Editor Version 5.00\n\n[HKEY_CURRENT_USER\\Software\\SimonTatham\\PuTTY\\Sessions\\web1]\n"HostName"="web1.example.com"\n…',
		parse: async (raw) => parsePutty(raw)
	},
	winscp: {
		id: 'winscp',
		app: 'WinSCP',
		accept: '.ini,.reg',
		tag: 'winscp-import',
		local: isWindows(),
		placeholder: '[Sessions\\Servers/web1]\nHostName=web1.example.com\nUserName=deploy\n…',
		parse: async (raw) => parseWinScp(raw)
	},
	tabby: {
		id: 'tabby',
		app: 'Tabby',
		accept: '.yaml,.yml,.json',
		tag: 'tabby-import',
		local: true,
		placeholder: 'profiles:\n  - type: ssh\n    name: web1\n    options:\n      host: web1.example.com\n      user: deploy\n…',
		parse: async (raw, password) => {
			let data: unknown;
			try {
				data = await tabbyData(raw);
			} catch (e) {
				return { ok: false, error: 'bad_file', detail: e instanceof Error ? e.message : String(e) };
			}
			return parseTabby(data, password);
		}
	}
};

/** The sources picked in the "another app" dialog, in menu order. */
export const OTHER_SOURCES: ImportSourceId[] = ['mremoteng', 'mobaxterm', 'putty', 'winscp', 'tabby'];

/** The app's settings on this computer as text, or null when none were found. */
export async function readLocal(source: ImportSourceId): Promise<string | null> {
	let parts = await invoke<number[][] | null>('import_read_local', { source });
	// mRemoteNG's predecessor keeps the same file elsewhere.
	if (!parts && source === 'mremoteng') parts = await invoke<number[][] | null>('import_read_local', { source: 'mremote' });
	if (!parts || parts.length === 0) return null;
	return parts.map((p) => decodeFile(Uint8Array.from(p))).join('\n');
}
