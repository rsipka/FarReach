import { isMobile } from '#lib/platform.js';

/** Whether this device can scan another device's link QR code: phones only. */
export function canScanLinkCode(): boolean {
	return isMobile();
}

/** Open the camera and read a link QR code. `null` when cancelled. */
export async function scanLinkCode(): Promise<string | null> {
	const scanner = await import('@tauri-apps/plugin-barcode-scanner');
	let permission = await scanner.checkPermissions();
	if (permission !== 'granted') permission = await scanner.requestPermissions();
	if (permission !== 'granted') throw new Error('Camera access was not allowed');
	const result = await scanner.scan({ windowed: false, formats: [scanner.Format.QRCode] });
	return result?.content?.trim() || null;
}
