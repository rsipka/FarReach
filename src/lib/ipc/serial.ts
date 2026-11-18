import { invoke } from '@tauri-apps/api/core';

/** A serial session's settings (see src-tauri/src/serial/options.rs). The
 *  port itself is the session's `host`. */
export interface SerialOptions {
	baud_rate: number;
	/** 5 to 8. */
	data_bits: number;
	parity: 'none' | 'odd' | 'even';
	/** 1 or 2. */
	stop_bits: number;
	/** software: XON/XOFF; hardware: RTS/CTS. */
	flow_control: 'none' | 'software' | 'hardware';
	/** What Enter sends. */
	newline: 'cr' | 'lf' | 'crlf';
	/** What Backspace sends. */
	backspace: 'del' | 'ctrlh';
	/** Show what is typed, for devices that do not echo it. */
	local_echo: boolean;
}

export const DEFAULT_SERIAL_OPTIONS: SerialOptions = {
	baud_rate: 9600,
	data_bits: 8,
	parity: 'none',
	stop_bits: 1,
	flow_control: 'none',
	newline: 'cr',
	backspace: 'del',
	local_echo: false
};

/** Speeds offered in the editor; any other can be typed. */
export const SERIAL_BAUD_RATES = [300, 1200, 2400, 4800, 9600, 19200, 38400, 57600, 115200, 230400, 460800, 921600];

/** "9600 8N1", as consoles are usually labelled. */
export function serialSummary(o: Partial<SerialOptions> | null | undefined): string {
	const s = { ...DEFAULT_SERIAL_OPTIONS, ...(o ?? {}) };
	return `${s.baud_rate} ${s.data_bits}${s.parity[0].toUpperCase()}${s.stop_bits}`;
}

export interface SerialPortInfo {
	name: string;
	/** What is plugged in, as far as the system says; may be empty. */
	description: string;
}

export interface SerialConnectParams {
	/** COM3, /dev/ttyUSB0, /dev/cu.usbserial-1410. */
	port: string;
	options: SerialOptions;
}

export async function serialListPorts(): Promise<SerialPortInfo[]> {
	return invoke<SerialPortInfo[]>('serial_list_ports');
}

/** Open the port; output then arrives as `serial-data-{id}` until `serial-exit-{id}`. */
export async function serialConnect(id: string, params: SerialConnectParams): Promise<void> {
	return invoke('serial_connect', { params: { id, port: params.port, options: params.options } });
}

export async function serialSend(id: string, data: number[]): Promise<void> {
	return invoke('serial_send', { id, data });
}

/** Hold a break on the line. */
export async function serialBreak(id: string): Promise<void> {
	return invoke('serial_break', { id });
}

export async function serialDisconnect(id: string): Promise<void> {
	return invoke('serial_disconnect', { id });
}
