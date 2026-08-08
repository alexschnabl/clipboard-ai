<!-- lib/components/HotkeyRecorder.svelte — press-to-record control for a command's hotkey -->
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onDestroy } from 'svelte';
	import { hotkeyFromEvent } from '$lib/hotkey';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { Problem } from '$lib/types';

	let {
		value,
		onchange,
		id
	}: { value: string; onchange: (v: string) => void; id?: string } = $props();

	let recording = $state(false);
	let captured = $state<string | null>(null);
	let hint = $state<string | null>(null);

	// Tracks the in-flight suspend_hotkeys() call. Every exit path waits on
	// this before calling resume_hotkeys(): stop() (or the onDestroy
	// backstop below) can otherwise run while suspend_hotkeys is still
	// in-flight, and its own invoke('resume_hotkeys') could then reach Rust
	// before the earlier suspend does. If those two land out of order the
	// resume is immediately undone by the suspend that follows it, and the
	// app is left with no working hotkeys and no visible reason -- the
	// exact failure this component exists to prevent.
	let suspending: Promise<void> | null = null;

	async function start() {
		if (recording) return;
		recording = true;
		captured = null;
		hint = null;
		// The app's own shortcuts must be off, or recording a combination that is
		// already assigned fires a transform instead of capturing it.
		suspending = invoke<void>('suspend_hotkeys')
			.catch((e) => {
				hint = String(e);
			})
			.finally(() => {
				suspending = null;
			});
		await suspending;
	}

	async function stop(commit: boolean): Promise<void> {
		if (!recording) return;
		recording = false;
		if (commit && captured) onchange(captured);
		captured = null;
		await resume();
	}

	/**
	 * Every exit path funnels through here — commit, cancel/Escape, blur and
	 * the onDestroy backstop below all call it. Waits out any in-flight
	 * suspend first (see `suspending`), then resumes unconditionally,
	 * including when suspend_hotkeys itself failed: resume_hotkeys just
	 * re-applies the saved config, which is safe whether or not the suspend
	 * actually took effect, and it is the only way back to working hotkeys.
	 *
	 * `resume_hotkeys` returns `Vec<Problem>` directly, not a `Result` — it
	 * essentially never rejects, so a re-registration failure (e.g. rediscovering
	 * an already-in-use hotkey) was previously discarded silently. Routing it
	 * into `configState.problems`, the same channel `save()` uses, is what makes
	 * it visible: unlike the local `hint` below, it survives this component
	 * unmounting, which matters because the onDestroy backstop calls this too.
	 */
	async function resume(): Promise<void> {
		if (suspending) await suspending;
		try {
			configState.problems = await invoke<Problem[]>('resume_hotkeys');
		} catch (e) {
			hint = String(e);
		}
	}

	/**
	 * This component is nested inside `CommandForm`'s `Dialog.Content`. bits-ui's
	 * `EscapeLayer` listens for Escape on `document`, in the bubble phase, and
	 * doesn't stopPropagation itself (verified in
	 * node_modules/bits-ui/dist/bits/utilities/escape-layer/use-escape-layer.svelte.js) --
	 * so a listener added here via `<svelte:window>` runs too late: `document`
	 * is reached before `window` on the way back up, meaning EscapeLayer's own
	 * unconditional `handleClose()` fires first and closes the whole dialog,
	 * discarding whatever the user has typed. Attaching directly to `document`,
	 * and in the *capture* phase, puts this handler ahead of EscapeLayer's in
	 * the event's path (capture runs root-to-target before bubble runs back up),
	 * so `stopPropagation()` below can retire the event before EscapeLayer's
	 * listener ever sees it.
	 *
	 * Only unmapped/mapped keys the recorder actually consumes get
	 * `preventDefault`/`stopPropagation` -- everything else (most notably Tab)
	 * is left alone so native focus navigation still works; the button's
	 * `onblur` handler ends recording cleanly once focus actually leaves it.
	 */
	function onkeydown(event: KeyboardEvent) {
		if (!recording) return;
		if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			void stop(false);
			return;
		}
		const mapped = hotkeyFromEvent(event);
		if (mapped) {
			event.preventDefault();
			event.stopPropagation();
			captured = mapped;
			hint = null;
			void stop(true);
		} else {
			hint = i18n.t.hotkeyRecorder.hintModifier;
		}
	}

	// Scoped to the recording lifecycle rather than a persistent listener: attaches
	// only while `recording` is true, and the cleanup below runs both on the next
	// effect re-run (recording flips false) and on unmount, so it can never leak
	// past this component being torn down.
	$effect(() => {
		if (!recording) return;
		document.addEventListener('keydown', onkeydown, { capture: true });
		return () => document.removeEventListener('keydown', onkeydown, { capture: true });
	});

	// Backstop for the one exit path the handlers above cannot see: the
	// component being torn down mid-recording. The settings window is only
	// ever hidden, never destroyed, and Rust re-applies the config on that
	// hide as a defensive fallback (see the CloseRequested handler in
	// src-tauri/src/lib.rs) — but this component can still unmount inside a
	// live window, e.g. the surrounding CommandForm dialog closing (Cancel,
	// its own close button, or an Escape that a nested dismissable layer
	// consumes before this component's own keydown handler sees it). If that
	// happens, `resume()` still runs; any problems it resolves with still
	// reach `configState.problems` (see `resume()` above), though an
	// IPC-level failure can only fall back to the local `hint`, and there is
	// no UI left to show that on.
	onDestroy(() => {
		if (!recording) return;
		recording = false;
		void resume();
	});
</script>

<button
	{id}
	type="button"
	class="w-full rounded-md border px-3 py-1.5 text-left font-mono text-sm"
	class:ring-2={recording}
	onclick={start}
	onblur={() => void stop(false)}
	aria-pressed={recording}
	aria-label={recording
		? i18n.t.hotkeyRecorder.recordingStatus
		: value
			? i18n.t.hotkeyRecorder.ariaHotkey(value)
			: i18n.t.hotkeyRecorder.ariaEmpty}
>
	{recording ? i18n.t.hotkeyRecorder.prompt : value || i18n.t.hotkeyRecorder.placeholder}
</button>
{#if recording}
	<p class="sr-only" role="status">
		{i18n.t.hotkeyRecorder.recordingStatus}
	</p>
{/if}
{#if hint}
	<p class="mt-1 text-xs text-muted-foreground" role="status">{hint}</p>
{/if}
