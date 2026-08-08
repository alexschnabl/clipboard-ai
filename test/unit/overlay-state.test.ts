// test/unit/overlay-state.test.ts — the overlay route's pure state
// vocabulary (src/routes/overlay/state.ts). No Svelte, no Tauri mocking
// needed: this is exactly the logic worth pinning without a webview.
import { describe, expect, it } from 'vitest';

import {
	FAILURE_WATCHDOG_MS,
	isKnownState,
	isTerminalState,
	OVERLAY_STATES
} from '../../src/routes/overlay/state';

describe('isKnownState', () => {
	it('accepts every declared overlay state', () => {
		for (const state of OVERLAY_STATES) {
			expect(isKnownState(state)).toBe(true);
		}
	});

	it('rejects an unrecognised payload instead of guessing which known state it means', () => {
		expect(isKnownState('progressing')).toBe(false);
		expect(isKnownState('Working')).toBe(false); // case-sensitive, no silent normalisation
		expect(isKnownState('')).toBe(false);
	});
});

describe('isTerminalState', () => {
	it('treats "failure" and "aborted" as terminal', () => {
		expect(isTerminalState('failure')).toBe(true);
		expect(isTerminalState('aborted')).toBe(true);
	});

	it('does not treat "working" or "escalated" as terminal, even though a cold local model can hold either for many seconds', () => {
		// A regression that made either of these terminal would let the client
		// watchdog auto-hide a still-running cycle's indicator out from under
		// it -- see FAILURE_WATCHDOG_MS's doc comment.
		expect(isTerminalState('working')).toBe(false);
		expect(isTerminalState('escalated')).toBe(false);
	});

	it('does not treat "hidden" as terminal', () => {
		expect(isTerminalState('hidden')).toBe(false);
	});
});

describe('FAILURE_WATCHDOG_MS', () => {
	it('leaves a generous margin over the Rust-side failure-visible duration (1500ms)', () => {
		// Not a duration this file owns -- see the doc comment on the
		// constant -- but a regression that shrinks it to e.g. 1500ms flat
		// would make the client-side watchdog race the very Rust-side timer
		// it's supposed to be a fallback for.
		expect(FAILURE_WATCHDOG_MS).toBeGreaterThan(1500);
	});
});
