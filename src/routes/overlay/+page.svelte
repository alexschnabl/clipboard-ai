<!--
	routes/overlay/+page.svelte — the in-flight indicator itself.

	Self-sufficient by design: this component owns no cross-cycle state of
	its own and knows nothing about hotkeys, backends or timing budgets — it
	just reflects the last `overlay://state` event it received from
	src-tauri/src/overlay.rs (see that module's doc comment for the full
	contract). Positioning, showing and hiding the *window* all happen on the
	Rust side; this route only ever decides what to draw inside it.

	Every render path defaults to nothing visible: `state` starts `'hidden'`,
	an unrecognised payload is ignored rather than guessed at (`isKnownState`),
	and the failure watchdog below bounds how long a "failure" can stay on
	screen even if the `"hidden"` that's supposed to follow it never arrives.
	None of this is a substitute for the Rust-side guarantee (the *window*
	itself gets hidden unconditionally, including on a panic — see
	`CycleOverlay`'s `Drop`) — it's defense in depth for the fact that a
	swallowed `window.hide()` error is still silently possible.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { FAILURE_WATCHDOG_MS, isKnownState, type OverlayState } from './state';

	let state = $state<OverlayState>('hidden');
	let failureWatchdog: ReturnType<typeof setTimeout> | undefined;

	function clearWatchdog() {
		if (failureWatchdog !== undefined) {
			clearTimeout(failureWatchdog);
			failureWatchdog = undefined;
		}
	}

	function apply(payload: string) {
		if (!isKnownState(payload)) return;
		clearWatchdog();
		state = payload;
		if (payload === 'failure') {
			failureWatchdog = setTimeout(() => {
				state = 'hidden';
			}, FAILURE_WATCHDOG_MS);
		}
	}

	onMount(() => {
		const unlisten = listen<{ state: string }>('overlay://state', (event) => {
			apply(event.payload.state);
		});
		return () => {
			clearWatchdog();
			void unlisten.then((f) => f());
		};
	});
</script>

<div class="stage" class:visible={state !== 'hidden'}>
	{#if state === 'working'}
		<span class="glyph dot" aria-hidden="true"></span>
	{:else if state === 'escalated'}
		<span class="glyph ring" aria-hidden="true"></span>
	{:else if state === 'failure'}
		<span class="glyph fail" aria-hidden="true">!</span>
	{/if}
</div>

<style>
	.stage {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100vw;
		height: 100vh;
		/* Opacity, not display:none — the window's own show/hide (Rust side)
		   is what actually gates visibility; this transition is purely
		   cosmetic polish for the instant the window *is* visible. */
		opacity: 0;
		transition: opacity 120ms ease-out;
	}

	.stage.visible {
		opacity: 1;
	}

	.glyph {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 50%;
		box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
	}

	/* Quiet state (0.25s–2.5s from cycle start): a small dark dot, legible
	   against arbitrary desktop content behind it. */
	.dot {
		width: 14px;
		height: 14px;
		background: rgba(28, 28, 32, 0.9);
		animation: pulse 1.1s ease-in-out infinite;
	}

	/* Escalated state (past 2.5s): bigger, amber, spinning — deliberately a
	   different shape and color from .dot, not just a resize, so a 16s cold
	   start reads as "still going, and now clearly the slow case" rather
	   than an identical dot that's been on screen a while. */
	.ring {
		width: 26px;
		height: 26px;
		background: transparent;
		border: 3px solid rgba(217, 142, 33, 0.35);
		border-top-color: rgb(217, 142, 33);
		animation: spin 0.8s linear infinite;
	}

	/* Failure/abort: distinct red glyph, no animation — it isn't "still
	   working", so it shouldn't look like it. Auto-clears via the Rust-side
	   timer (overlay.rs) plus the client-side watchdog above. */
	.fail {
		width: 22px;
		height: 22px;
		background: rgb(199, 58, 58);
		color: white;
		font: 700 14px/1 system-ui, sans-serif;
	}

	@keyframes pulse {
		0%,
		100% {
			transform: scale(1);
			opacity: 0.85;
		}
		50% {
			transform: scale(1.15);
			opacity: 1;
		}
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
