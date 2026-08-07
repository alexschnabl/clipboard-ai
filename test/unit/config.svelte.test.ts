// test/unit/config.svelte.test.ts — dirty-state logic for ConfigState.
//
// Named `*.svelte.test.ts`, not plain `*.test.ts`: the source under test
// (`src/lib/state/config.svelte.ts`) uses runes ($state), which only
// compile correctly through the Svelte plugin registered in
// vitest.config.ts, and that plugin's module filter recognises files
// containing the `.svelte.` infix (see `DEFAULT_SVELTE_MODULE_INFIX` in
// @sveltejs/vite-plugin-svelte) -- this file's own name included, so it can
// use runes too if a future test needs to.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { ConfigState } from '../../src/lib/state/config.svelte';
import type { BackendConfig, CommandConfig } from '../../src/lib/types';

function backend(overrides: Partial<BackendConfig> = {}): BackendConfig {
	return {
		id: 'b1',
		name: 'Backend',
		kind: 'claude-cli',
		baseUrl: null,
		model: null,
		apiKey: null,
		timeoutMs: 120_000,
		...overrides
	};
}

function command(overrides: Partial<CommandConfig> = {}): CommandConfig {
	return {
		id: 'c1',
		name: 'Command',
		backendId: 'b1',
		hotkey: 'Ctrl+Alt+J',
		systemPrompt: 'Do a thing.',
		...overrides
	};
}

describe('ConfigState dirty-state logic', () => {
	let state: ConfigState;

	beforeEach(() => {
		invokeMock.mockReset();
		state = new ConfigState();
	});

	it('starts clean', () => {
		expect(state.dirty).toBe(false);
	});

	it('touch() marks the state dirty', () => {
		state.touch();
		expect(state.dirty).toBe(true);
	});

	it('addBackend, updateBackend and a successful deleteBackend each mark the state dirty', () => {
		state.addBackend(backend());
		expect(state.dirty).toBe(true);

		state.dirty = false;
		state.updateBackend(backend({ name: 'Renamed' }));
		expect(state.dirty).toBe(true);

		state.dirty = false;
		state.deleteBackend('b1');
		expect(state.dirty).toBe(true);
	});

	it('addCommand, updateCommand and deleteCommand each mark the state dirty', () => {
		state.config.backends.push(backend());

		state.addCommand(command());
		expect(state.dirty).toBe(true);

		state.dirty = false;
		state.updateCommand(command({ name: 'Renamed' }));
		expect(state.dirty).toBe(true);

		state.dirty = false;
		state.deleteCommand('c1');
		expect(state.dirty).toBe(true);
	});

	it('updateCommand is a no-op, and does not mark the state dirty, for an id that does not exist', () => {
		state.dirty = false;

		state.updateCommand(command({ id: 'does-not-exist' }));

		expect(state.dirty).toBe(false);
		expect(state.config.commands).toHaveLength(0);
	});

	it('a refused deleteBackend (still used by a command) does not mark the state dirty', () => {
		state.config.backends.push(backend());
		state.config.commands.push(command());
		state.dirty = false;

		const result = state.deleteBackend('b1');

		expect(result.ok).toBe(false);
		expect(result.usedBy).toEqual(['Command']);
		expect(state.config.backends).toHaveLength(1);
		expect(state.dirty).toBe(false);
	});

	it('save() clears dirty on a successful save', async () => {
		state.touch();
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [] });

		const ok = await state.save();

		expect(ok).toBe(true);
		expect(state.dirty).toBe(false);
	});

	it('save() leaves dirty set when the backend rejects the config', async () => {
		state.touch();
		invokeMock.mockResolvedValue({ ok: false, error: 'invalid config', problems: [] });

		const ok = await state.save();

		expect(ok).toBe(false);
		expect(state.dirty).toBe(true);
	});

	it('save() leaves dirty set when the invoke call itself throws', async () => {
		state.touch();
		invokeMock.mockRejectedValue(new Error('ipc failed'));

		const ok = await state.save();

		expect(ok).toBe(false);
		expect(state.dirty).toBe(true);
		expect(state.error).toContain('ipc failed');
	});

	it('load() resets dirty to false, discarding any unsaved edits', async () => {
		state.touch();
		invokeMock.mockResolvedValue({
			config: { version: 2, backends: [], commands: [] },
			loadError: null
		});

		await state.load();

		expect(state.dirty).toBe(false);
	});

	it('load() bumps revision on success, so a Discard reload also resets transient UI state', async () => {
		const before = state.revision;
		invokeMock.mockResolvedValue({
			config: { version: 2, backends: [], commands: [] },
			loadError: null
		});

		await state.load();

		expect(state.revision).toBe(before + 1);
	});
});
