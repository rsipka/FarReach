import assert from 'node:assert/strict';
import { mockIPC, clearMocks } from '@tauri-apps/api/mocks';
import { invokeProject, invalidateProjectRequests, projectSummary } from '../src/lib/ipc/devops-projects.ts';

globalThis.window = {};
try {
	const pending = Promise.withResolvers();
	mockIPC(() => pending.promise);
	const read = invokeProject('tofu_open_project');
	invalidateProjectRequests();
	pending.resolve({ encryption: { passphrase: 'old identity' } });
	await assert.rejects(read, /vault changed/);

	const summary = { id: 'a', name: 'A', path: '/a', description: '', createdAt: '', lastOpenedAt: '' };
	mockIPC(() => [summary]);
	assert.deepEqual(await invokeProject('ansible_list_projects'), [summary]);
	assert.deepEqual(projectSummary({ ...summary, vaultPassword: 'secret', inventoryHosts: [] }), summary);
} finally {
	clearMocks();
}
