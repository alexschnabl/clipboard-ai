// routes/overlay/state.ts — the overlay's visual vocabulary, kept as plain
// data/functions (no Svelte, no Tauri) so "is this a state we know how to
// draw" and "how long is the failure watchdog" are unit-testable without a
// webview. `+page.svelte` is the only consumer; this file has no reason to
// live anywhere else.

/** Mirrors the `state` field of the `overlay://state` payload emitted by
 *  `src-tauri/src/overlay.rs`'s `emit_to`. `hidden` is both the initial
 *  local value (nothing renders before the first real event) and the
 *  terminal one every cycle is promised to reach -- see that module's doc
 *  comment for the guard/Drop guarantee behind that promise. */
export const OVERLAY_STATES = ['hidden', 'working', 'escalated', 'failure'] as const;
export type OverlayState = (typeof OVERLAY_STATES)[number];

/** An event payload this route doesn't recognise is treated as "ignore",
 *  never "guess" -- the same abort-rather-than-assume instinct `cycle.rs`
 *  applies to a missing `<output>` sentinel. Keeps a future new state or a
 *  version skew between frontend and backend from rendering nonsense. */
export function isKnownState(value: string): value is OverlayState {
	return (OVERLAY_STATES as readonly string[]).includes(value);
}

/** `overlay.rs` promises a `"failure"` state is always followed by
 *  `"hidden"` within its own `FAILURE_VISIBLE_MS` (1500ms at the time of
 *  writing). This is a client-side fallback for that promise being broken
 *  (e.g. a swallowed `window.hide()` error) -- generous margin over it, not
 *  a duration this file is the source of truth for. Deliberately does not
 *  apply to `"working"`/`"escalated"`: a cold local model legitimately runs
 *  for many seconds (16s in this project's own spike), so there is no fixed
 *  bound those two states could be auto-cleared on without risking clearing
 *  a real, still-running cycle's indicator out from under it. */
export const FAILURE_WATCHDOG_MS = 4000;
