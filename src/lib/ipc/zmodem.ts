import { invoke } from '@tauri-apps/api/core';

/** Which way the files go, from this side: upload when the remote ran rz,
 *  download when it ran sz. */
export type ZmodemDirection = 'upload' | 'download';

/** Told as `zmodem-{connectionId}` by SSH, Telnet and serial sessions
 *  (see src-tauri/src/zmodem). */
export type ZmodemEvent =
	| { kind: 'offer'; direction: ZmodemDirection }
	| {
			kind: 'progress';
			direction: ZmodemDirection;
			file: string;
			/** From 1. */
			index: number;
			/** Known for an upload. */
			count: number | null;
			done: number;
			total: number | null;
	  }
	| { kind: 'finished'; direction: ZmodemDirection; files: string[]; folder: string | null }
	| { kind: 'cancelled'; direction: ZmodemDirection }
	| { kind: 'failed'; direction: ZmodemDirection; error: string };

/** The remote ran rz: send these files. */
export async function zmodemUpload(id: string, paths: string[]): Promise<void> {
	return invoke('zmodem_upload', { id, paths });
}

/** The remote ran sz: save into this folder. */
export async function zmodemDownload(id: string, folder: string): Promise<void> {
	return invoke('zmodem_download', { id, folder });
}

/** Decline the transfer, or stop the one running. */
export async function zmodemCancel(id: string): Promise<void> {
	return invoke('zmodem_cancel', { id });
}
