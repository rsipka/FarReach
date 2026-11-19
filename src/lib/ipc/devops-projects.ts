import { invoke } from '@tauri-apps/api/core';

export interface ProjectSummary {
	id: string;
	name: string;
	path: string;
	description: string;
	createdAt: string;
	lastOpenedAt: string;
}

// A response issued before lock or identity replacement may reach the webview
// after the project workspace has been cleared.
let generation = 0;
export const projectGeneration = () => generation;
export function invalidateProjectRequests(): void {
	generation++;
}

export async function invokeProject<T>(command: string, args?: Record<string, unknown>): Promise<T> {
	const started = generation;
	const result = await invoke<T>(command, args);
	if (generation !== started) throw new Error('The vault changed during this project operation');
	return result;
}

export function projectSummary({ id, name, path, description, createdAt, lastOpenedAt }: ProjectSummary): ProjectSummary {
	return { id, name, path, description, createdAt, lastOpenedAt };
}
