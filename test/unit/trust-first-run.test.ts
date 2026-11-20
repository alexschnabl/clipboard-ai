import { beforeEach, describe, expect, it, vi } from 'vitest';

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock('@tauri-apps/api/core', () => ({ invoke: invokeMock }));

import { ConfigState } from '../../src/lib/state/config.svelte';
import { HealthState, isWorking } from '../../src/lib/state/health.svelte';
import { OnboardingState } from '../../src/lib/state/onboarding.svelte';
import {
	cliVersion,
	formatSeconds,
	healthSentence,
	healthTone,
	resultSentence
} from '../../src/lib/health-format';
import { classifyCommandProblem, commandProblemSentence } from '../../src/lib/problem-format';
import { en } from '../../src/lib/i18n/en';
import { registry } from '../../src/lib/i18n/registry';
import type { BackendConfig, Health } from '../../src/lib/types';

function backend(overrides: Partial<BackendConfig> = {}): BackendConfig {
	return {
		id: 'b1',
		name: 'Backend',
		kind: 'anthropic-api',
		baseUrl: null,
		model: null,
		apiKey: null,
		timeoutMs: 120_000,
		effort: 'low',
		expectsResponse: true,
		...overrides
	};
}

beforeEach(() => invokeMock.mockReset());

describe('health sentences', () => {
	it('formats seconds with one decimal under ten seconds and none above', () => {
		expect(formatSeconds(1140, 'en')).toBe('1.1 s');
		expect(formatSeconds(16_036, 'en')).toBe('16 s');
		expect(formatSeconds(1140, 'de')).toBe('1,1 s');
	});

	it('reads the version number out of the CLI banner', () => {
		expect(cliVersion('2.1.329 (Claude Code)')).toBe('2.1.329');
	});

	it('gives every status one plain sentence in every language', () => {
		const samples: Health[] = [
			{ status: 'ok', ms: 1100 },
			{ status: 'cold', firstMs: 16_000, warmMs: 900 },
			{ status: 'slow', ms: 9000 },
			{ status: 'reachable', ms: 3 },
			{ status: 'cliReady', version: '2.1.0' },
			{ status: 'cliSignedOut', version: '2.1.0' },
			{ status: 'cliUnknownAuth', version: '1.0.0' },
			{ status: 'cliMissing' },
			{ status: 'keyMissing' },
			{ status: 'keyRejected' },
			{ status: 'modelNotFound' },
			{ status: 'notReachable', detail: 'refused' },
			{ status: 'timeout', secs: 60 },
			{ status: 'misconfigured', detail: 'baseUrl is required' },
			{ status: 'badResponse', detail: 'HTTP 500' }
		];
		for (const { dictionary } of Object.values(registry)) {
			for (const health of samples) {
				const sentence = healthSentence(health, dictionary.health);
				expect(sentence.length).toBeGreaterThan(0);
				expect(sentence).not.toMatch(/[–—]/);
			}
		}
		expect(healthSentence({ status: 'ok', ms: 1100 }, en.health, 'en')).toBe('Answered in 1.1 s');
		expect(healthSentence({ status: 'keyRejected' }, en.health)).toBe('Key rejected');
		expect(
			healthSentence({ status: 'misconfigured', detail: 'model is required' }, en.health)
		).toBe('No model set');
	});

	it('maps statuses to dot tones', () => {
		expect(healthTone(undefined)).toBe('idle');
		expect(healthTone(undefined, true)).toBe('busy');
		expect(healthTone({ health: { status: 'ok', ms: 1 }, error: null })).toBe('ok');
		expect(healthTone({ health: { status: 'slow', ms: 9000 }, error: null })).toBe('warn');
		expect(healthTone({ health: { status: 'keyRejected' }, error: null })).toBe('error');
		expect(healthTone({ health: null, error: 'ipc' })).toBe('error');
		expect(resultSentence(undefined, false, en.health)).toBe('Not checked yet');
	});
});

describe('command problems', () => {
	it('turns the Rust hotkey messages into one plain sentence', () => {
		const taken = classifyCommandProblem(
			'Fix grammar: hotkey "Ctrl+Alt+J" is already in use (HotKey already registered)',
			'Fix grammar'
		);
		expect(commandProblemSentence(taken, en.commands.problems)).toBe(
			'Another app already uses this hotkey. Pick a different one.'
		);
		const clash = classifyCommandProblem(
			'Second: hotkey "Control+Alt+J" collides with "First"\'s hotkey after parsing (x); only "First" is registered',
			'Second'
		);
		expect(clash).toEqual({ kind: 'hotkeyClash', other: 'First' });
		expect(classifyCommandProblem('Bad: unparseable hotkey "" (empty)', 'Bad').kind).toBe(
			'hotkeyInvalid'
		);
		expect(classifyCommandProblem('Orphan: unknown backend', 'Orphan').kind).toBe(
			'backendMissing'
		);
		expect(
			classifyCommandProblem('X: backend is misconfigured: could not read the API key', 'X')
		).toEqual({ kind: 'other', text: 'Backend is misconfigured: could not read the API key' });
	});
});

describe('API keys in the config state', () => {
	it('sends a typed key as a key change and never inside the config', async () => {
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [], storedKeys: ['b1'] });
		const state = new ConfigState();

		await state.addBackend(backend({ apiKey: 'sk-should-not-travel' }), 'sk-new');

		const [, payload] = invokeMock.mock.calls[0]!;
		expect(payload.config.backends[0].apiKey).toBeNull();
		expect(payload.keyChanges).toEqual({ b1: 'sk-new' });
		expect(state.hasStoredKey('b1')).toBe(true);
	});

	it('leaves keyChanges out entirely when the key was not touched', async () => {
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [], storedKeys: ['b1'] });
		const state = new ConfigState();
		state.config.backends.push(backend());

		await state.updateBackend(backend({ name: 'Renamed' }));

		const [, payload] = invokeMock.mock.calls[0]!;
		expect(payload).not.toHaveProperty('keyChanges');
	});

	it('sends null to remove a key', async () => {
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [], storedKeys: [] });
		const state = new ConfigState();
		state.storedKeys = ['b1'];
		state.config.backends.push(backend());

		await state.updateBackend(backend(), null);

		expect(invokeMock.mock.calls[0]![1].keyChanges).toEqual({ b1: null });
		expect(state.hasStoredKey('b1')).toBe(false);
	});

	it('loads stored key ids and the default-setup flag', async () => {
		invokeMock.mockResolvedValue({
			config: {
				version: 2,
				backends: [],
				commands: [],
				settings: { theme: 'system', language: 'system', autostart: false }
			},
			loadError: null,
			startupProblems: [],
			storedKeys: ['x'],
			isDefaultSetup: true
		});
		const state = new ConfigState();
		await state.load();
		expect(state.storedKeys).toEqual(['x']);
		expect(state.isDefaultSetup).toBe(true);
	});

	it('names a deleted backend explicitly so only its credential is removed', async () => {
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [], storedKeys: [] });
		const state = new ConfigState();
		state.config.backends.push(backend());

		await state.deleteBackend('b1');
		await state.updateSettings({ ...state.config.settings, theme: 'dark' });

		expect(invokeMock.mock.calls[0]![1].removedBackends).toEqual(['b1']);
		expect(invokeMock.mock.calls[1]![1]).not.toHaveProperty('removedBackends');
	});

	it('persists the onboarding dismissal in settings', async () => {
		invokeMock.mockResolvedValue({ ok: true, error: null, problems: [] });
		const state = new ConfigState();
		await state.dismissOnboarding();
		expect(invokeMock.mock.calls[0]![1].config.settings.onboardingDismissed).toBe(true);
	});
});

describe('health state', () => {
	it('caches results per backend and coalesces a second check of the same backend', async () => {
		let resolve!: (h: Health) => void;
		invokeMock.mockReturnValue(new Promise<Health>((r) => (resolve = r)));
		const health = new HealthState();

		const first = health.check('b1');
		const second = health.check('b1');
		expect(health.isChecking('b1')).toBe(true);
		resolve({ status: 'ok', ms: 900 });
		await Promise.all([first, second]);

		expect(invokeMock).toHaveBeenCalledTimes(1);
		expect(invokeMock).toHaveBeenCalledWith('check_backend', { backendId: 'b1' });
		expect(health.isChecking('b1')).toBe(false);
		expect(isWorking(health.results['b1'])).toBe(true);
		health.forget('b1');
		expect(health.results['b1']).toBeUndefined();
	});

	it('knows when every backend was checked and none works', async () => {
		invokeMock.mockResolvedValue({ status: 'notReachable', detail: 'refused' });
		const health = new HealthState();
		expect(health.allCheckedAndFailing(['a', 'b'])).toBe(false);
		await health.checkAll(['a', 'b']);
		expect(health.allCheckedAndFailing(['a', 'b'])).toBe(true);
		expect(health.anyWorking(['a', 'b'])).toBe(false);
	});

	it('records an IPC failure as a failed check instead of throwing', async () => {
		invokeMock.mockImplementationOnce(() => Promise.reject('no backend with id x'));
		const health = new HealthState();
		await health.check('x');
		expect(health.results['x']?.error).toContain('no backend');
	});
});

describe('getting started card', () => {
	it('shows on an untouched default setup until a run has succeeded', async () => {
		const onboarding = new OnboardingState();
		const input = { dismissed: false, isDefaultSetup: true, allBackendsFailing: false };
		expect(onboarding.shouldShow(input)).toBe(false);

		invokeMock.mockResolvedValue([]);
		await onboarding.refreshHistory();
		expect(onboarding.shouldShow(input)).toBe(true);
		expect(onboarding.shouldShow({ ...input, dismissed: true })).toBe(false);

		invokeMock.mockResolvedValue([{ outcome: { status: 'succeeded' } }]);
		await onboarding.refreshHistory();
		expect(onboarding.shouldShow(input)).toBe(false);
	});

	it('shows for a customised setup only when every backend failed its check', async () => {
		const onboarding = new OnboardingState();
		invokeMock.mockResolvedValue([]);
		await onboarding.refreshHistory();
		const custom = { dismissed: false, isDefaultSetup: false, allBackendsFailing: false };
		expect(onboarding.shouldShow(custom)).toBe(false);
		expect(onboarding.shouldShow({ ...custom, allBackendsFailing: true })).toBe(true);
	});

	it('stays up for the rest of the session once shown, until hidden', async () => {
		const onboarding = new OnboardingState();
		invokeMock.mockResolvedValue([]);
		await onboarding.refreshHistory();
		onboarding.sticky = true;
		const custom = { dismissed: false, isDefaultSetup: false, allBackendsFailing: false };
		expect(onboarding.shouldShow(custom)).toBe(true);
		onboarding.hiddenLocally = true;
		expect(onboarding.shouldShow(custom)).toBe(false);
	});
});

describe('new strings', () => {
	it('use no en or em dashes in any language', () => {
		for (const [code, { dictionary }] of Object.entries(registry)) {
			const sections = [
				dictionary.health,
				dictionary.update,
				dictionary.commands.problems,
				dictionary.commands.onboarding
			];
			const flat = JSON.stringify(sections, (_k, v) => (typeof v === 'function' ? v('X', 'Y') : v));
			expect(flat, code).not.toMatch(/[–—]/);
		}
	});
});
