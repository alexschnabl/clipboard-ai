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
	and the watchdog below bounds how long a terminal state can stay on
	screen even if the `"hidden"` that's supposed to follow it never arrives.
	None of this is a substitute for the Rust-side guarantee (the *window*
	itself gets hidden unconditionally, including on a panic — see
	`CycleOverlay`'s `Drop`) — it's defense in depth for the fact that a
	swallowed `window.hide()` error is still silently possible.

	Visual treatment is variation 1 ("pulsing dot") from
	.superpowers/mockups/indicators.html, chosen by the author over four
	alternatives. A 10px dot in a 28px window, and an *opacity* pulse rather
	than a size or rotation one: the window now follows the cursor, so the
	mark already has motion of its own. Anything that also span or resized
	would be compound movement over the document the user is reading, which
	is the thing this indicator must not become.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import {
		FAILURE_WATCHDOG_MS,
		isKnownState,
		isTerminalState,
		type OverlayState
	} from './state';

	let state = $state<OverlayState>('hidden');
	let watchdog: ReturnType<typeof setTimeout> | undefined;

	function clearWatchdog() {
		if (watchdog !== undefined) {
			clearTimeout(watchdog);
			watchdog = undefined;
		}
	}

	function apply(payload: string) {
		if (!isKnownState(payload)) return;
		clearWatchdog();
		state = payload;
		if (isTerminalState(payload)) {
			watchdog = setTimeout(() => {
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

<div class="stage" data-state={state}>
	{#if state !== 'hidden'}
		<span class="dot" aria-hidden="true"></span>
		{#if state === 'escalated'}
			<!-- The escalation cue is layered *under* the same unchanged pulse
			     rather than speeding it up: a faster tempo reads as alarm, and
			     a slow run is not an error. -->
			<span class="ping" aria-hidden="true"></span>
		{/if}
	{/if}
</div>

<style>
	.stage {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100vw;
		height: 100vh;
	}

	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		/* A light hairline immediately outside a dark one. The window floats
		   over arbitrary content — a white document, a black editor, a
		   photograph — so a single-colour mark would disappear against
		   something. Two rings of opposing lightness guarantee one of them
		   always breaks contrast with whatever is underneath. */
		box-shadow:
			0 0 0 1px rgba(255, 255, 255, 0.85),
			0 0 0 2px rgba(0, 0, 0, 0.38),
			0 1px 3px rgba(0, 0, 0, 0.28);
	}

	/* Teal (the app's --primary) while running, neutral grey when the cycle
	   declined to paste, red only when something genuinely failed. The
	   values are the light-theme tokens from app.css, hard-coded because
	   this window is transparent and sits over other applications: it has no
	   app background to inherit a theme from, and must look identical
	   whatever theme the settings window happens to be in. */
	.stage[data-state='working'] .dot,
	.stage[data-state='escalated'] .dot {
		background: oklch(0.511 0.096 186.391);
		animation: pulse 1.8s ease-in-out infinite;
	}

	.ping {
		position: absolute;
		width: 10px;
		height: 10px;
		border-radius: 50%;
		border: 1px solid oklch(0.511 0.096 186.391);
		animation: ping 2.6s ease-out infinite;
	}

	/* Aborted: the cycle deliberately declined to paste (focus moved, a
	   modifier was still held). That is correct behaviour, not a fault, so
	   it gets a neutral colour and a single calm fade — no flash. Reading
	   this as an error would misreport the app's most important guarantee. */
	.stage[data-state='aborted'] .dot {
		background: oklch(0.56 0.021 213.5);
		animation: settle 900ms ease-in-out forwards;
	}

	/* Failure: destructive red and a sharp double-flash, so it is
	   distinguishable at a glance from the calm fade above. */
	.stage[data-state='failure'] .dot {
		background: oklch(0.577 0.245 27.325);
		animation: alarm 1200ms ease-in-out forwards;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.42;
		}
	}

	@keyframes ping {
		0% {
			transform: scale(1);
			opacity: 0.55;
		}
		100% {
			transform: scale(2.6);
			opacity: 0;
		}
	}

	@keyframes settle {
		0%,
		45% {
			opacity: 1;
			transform: scale(1);
		}
		100% {
			opacity: 0;
			transform: scale(0.75);
		}
	}

	@keyframes alarm {
		0%,
		10% {
			opacity: 1;
			transform: scale(1);
		}
		20% {
			opacity: 0.3;
			transform: scale(1.15);
		}
		30% {
			opacity: 1;
			transform: scale(1);
		}
		40% {
			opacity: 0.3;
			transform: scale(1.15);
		}
		50%,
		70% {
			opacity: 1;
			transform: scale(1);
		}
		100% {
			opacity: 0;
			transform: scale(0.6);
		}
	}

	/* Someone who has asked the OS to reduce motion should not get a pulsing
	   dot over their document. The mark still appears and still carries its
	   state through colour — only the animation stops. */
	@media (prefers-reduced-motion: reduce) {
		.dot,
		.ping {
			animation: none !important;
		}
		.ping {
			display: none;
		}
	}
</style>
