// test/unit/config.svelte.test.ts — commit-and-rollback semantics for ConfigState.
//
// Named `*.svelte.test.ts`, not plain `*.test.ts`: the source under test
// (`src/lib/state/config.svelte.ts`) uses runes ($state), which only
// compile correctly through the Svelte plugin registered in
// vitest.config.ts, and that plugin's module filter recognises files
// containing the `.svelte.` infix (see `DEFAULT_SVELTE_MODULE_INFIX` in
// @sveltejs/vite-plugin-svelte) -- this file's own name included, so it can
// use runes too if a future test needs to.
//
// There is no more "dirty" concept to test: every mutation method commits
// through `save_config` immediately (see `ConfigState.commit`'s doc
// comment), so what matters here is the commit-or-rollback contract --
// a mutation applies to `config` right away, and a rejected save must undo
// it rather than leave the in-memory list showing something disk never got.
import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { ConfigState } from '../../src/lib/state/config.svelte';
import type { BackendConfig, CommandConfig, SaveResult } from '../../src/lib/types';

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

function ok(): SaveResult {
	return { ok: true, error: null, problems: [] };
}

function rejected(error = 'invalid config'): SaveResult {
	return { ok: false, error, problems: [] };
}

describe('ConfigState commit-and-rollback semantics', () => {
	let state: ConfigState;

	beforeEach(() => {
		invokeMock.mockReset();
		state = new ConfigState();
	});

	describe('addBackend', () => {
		it('applies the mutation and persists it via save_config on success', async () => {
			invokeMock.mockResolvedValue(ok());

			const result = await state.addBackend(backend());

			expect(result).toBe(true);
			expect(state.config.backends).toHaveLength(1);
			expect(invokeMock).toHaveBeenCalledTimes(1);
			const [command_, payload] = invokeMock.mock.calls[0]!;
			expect(command_).toBe('save_config');
			expect(payload.config.backends).toHaveLength(1);
		});

		it('rolls back the in-memory list when the backend rejects the save', async () => {
			invokeMock.mockResolvedValue(rejected('duplicate id: b1'));

			const result = await state.addBackend(backend());

			expect(result).toBe(false);
			expect(state.config.backends).toHaveLength(0);
			expect(state.error).toEqual({ message: 'duplicate id: b1', source: 'save' });
		});

		it('rolls back when the invoke call itself throws', async () => {
			invokeMock.mockRejectedValue(new Error('ipc failed'));

			const result = await state.addBackend(backend());

			expect(result).toBe(false);
			expect(state.config.backends).toHaveLength(0);
			expect(state.error?.message).toContain('ipc failed');
		});
	});

	describe('updateBackend', () => {
		it('applies the update and keeps it on a successful save', async () => {
			state.config.backends.push(backend());
			invokeMock.mockResolvedValue(ok());

			const result = await state.updateBackend(backend({ name: 'Renamed' }));

			expect(result).toBe(true);
			expect(state.config.backends[0]!.name).toBe('Renamed');
		});

		it('rolls back to the pre-edit backend when the save is rejected', async () => {
			state.config.backends.push(backend());
			invokeMock.mockResolvedValue(rejected());

			const result = await state.updateBackend(backend({ name: 'Renamed' }));

			expect(result).toBe(false);
			expect(state.config.backends[0]!.name).toBe('Backend');
		});
	});

	describe('deleteBackend', () => {
		it('refuses locally, before any write, while a command still uses it', async () => {
			state.config.backends.push(backend());
			state.config.commands.push(command());

			const result = await state.deleteBackend('b1');

			expect(result).toEqual({ ok: false, usedBy: ['Command'] });
			expect(state.config.backends).toHaveLength(1);
			expect(invokeMock).not.toHaveBeenCalled();
		});

		it('persists the delete when nothing references it', async () => {
			state.config.backends.push(backend());
			invokeMock.mockResolvedValue(ok());

			const result = await state.deleteBackend('b1');

			expect(result).toEqual({ ok: true, usedBy: [] });
			expect(state.config.backends).toHaveLength(0);
		});

		it('restores the backend when the write is rejected', async () => {
			state.config.backends.push(backend());
			invokeMock.mockResolvedValue(rejected());

			const result = await state.deleteBackend('b1');

			expect(result.ok).toBe(false);
			expect(state.config.backends).toHaveLength(1);
			expect(state.config.backends[0]!.id).toBe('b1');
		});
	});

	describe('addCommand', () => {
		it('applies and persists on success, then rolls back a later rejected add', async () => {
			state.config.backends.push(backend());
			invokeMock.mockResolvedValue(ok());
			expect(await state.addCommand(command())).toBe(true);
			expect(state.config.commands).toHaveLength(1);

			invokeMock.mockResolvedValue(rejected());
			expect(await state.addCommand(command({ id: 'c2' }))).toBe(false);
			expect(state.config.commands).toHaveLength(1);
			expect(state.config.commands[0]!.id).toBe('c1');
		});
	});

	describe('updateCommand', () => {
		it('is a no-op, with no IPC call at all, for an id that does not exist', async () => {
			const result = await state.updateCommand(command({ id: 'does-not-exist' }));

			expect(result).toBe(false);
			expect(state.config.commands).toHaveLength(0);
			expect(invokeMock).not.toHaveBeenCalled();
		});

		it('rolls back to the pre-edit command when the save is rejected', async () => {
			state.config.backends.push(backend());
			state.config.commands.push(command());
			invokeMock.mockResolvedValue(rejected());

			const result = await state.updateCommand(command({ name: 'Renamed' }));

			expect(result).toBe(false);
			expect(state.config.commands[0]!.name).toBe('Command');
		});
	});

	describe('deleteCommand', () => {
		it('persists the delete on success', async () => {
			state.config.backends.push(backend());
			state.config.commands.push(command());
			invokeMock.mockResolvedValue(ok());

			const result = await state.deleteCommand('c1');

			expect(result).toBe(true);
			expect(state.config.commands).toHaveLength(0);
		});

		it('restores the command when the write is rejected', async () => {
			state.config.backends.push(backend());
			state.config.commands.push(command());
			invokeMock.mockResolvedValue(rejected());

			const result = await state.deleteCommand('c1');

			expect(result).toBe(false);
			expect(state.config.commands).toHaveLength(1);
		});
	});

	describe('saving', () => {
		it('is true only while a commit is in flight', async () => {
			let resolveSave!: (value: SaveResult) => void;
			invokeMock.mockReturnValue(
				new Promise<SaveResult>((resolve) => {
					resolveSave = resolve;
				})
			);

			expect(state.saving).toBe(false);
			const pending = state.addBackend(backend());
			expect(state.saving).toBe(true);

			resolveSave(ok());
			await pending;

			expect(state.saving).toBe(false);
		});
	});

	describe('load()', () => {
		it('bumps revision and clears any previous error on success', async () => {
			state.error = { message: 'stale error', source: 'save' };
			const before = state.revision;
			invokeMock.mockResolvedValue({
				config: { version: 2, backends: [], commands: [] },
				loadError: null,
				startupProblems: []
			});

			await state.load();

			expect(state.revision).toBe(before + 1);
			expect(state.error).toBeNull();
		});

		it('sets error with source "load" on a failed load', async () => {
			invokeMock.mockRejectedValue(new Error('disk error'));

			await state.load();

			expect(state.error?.message).toContain('disk error');
			expect(state.error?.source).toBe('load');
		});

		it('loads startup problems into `problems`, the same channel save() uses', async () => {
			invokeMock.mockResolvedValue({
				config: { version: 2, backends: [], commands: [] },
				loadError: null,
				startupProblems: [{ commandId: 'c1', message: 'hotkey already in use' }]
			});

			await state.load();

			expect(state.problems).toEqual([{ commandId: 'c1', message: 'hotkey already in use' }]);
		});
	});
});
