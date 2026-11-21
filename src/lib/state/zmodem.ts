/** Asking a terminal tab to send files with ZMODEM: its ZmodemBar picks the
 *  files, types `rz` and sends them once rz answers. */

export const ZMODEM_SEND_EVENT = 'reach:zmodem-send';

/** The tab types whose terminals can run a ZMODEM transfer. */
export function canZmodem(type: string): boolean {
	return type === 'ssh' || type === 'telnet' || type === 'serial';
}

export function requestZmodemSend(tabId: string): void {
	window.dispatchEvent(new CustomEvent<string>(ZMODEM_SEND_EVENT, { detail: tabId }));
}
