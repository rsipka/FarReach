/** Asking a terminal tab for a ZMODEM transfer from its bar or menu: its
 *  ZmodemBar picks what to send or where to save, types rz or sz, and
 *  answers the transfer when it starts. */

export const ZMODEM_SEND_EVENT = 'reach:zmodem-send';

/** files / folder: send them (rz). download-folder: fetch the remote's
 *  current folder, all levels (sz from find). */
export type ZmodemRequestKind = 'files' | 'folder' | 'download-folder';

export interface ZmodemRequest {
	tabId: string;
	kind: ZmodemRequestKind;
}

/** The tab types whose terminals can run a ZMODEM transfer. */
export function canZmodem(type: string): boolean {
	return type === 'ssh' || type === 'telnet' || type === 'serial';
}

export function requestZmodem(tabId: string, kind: ZmodemRequestKind = 'files'): void {
	window.dispatchEvent(new CustomEvent<ZmodemRequest>(ZMODEM_SEND_EVENT, { detail: { tabId, kind } }));
}
