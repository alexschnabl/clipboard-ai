<!-- lib/components/HotkeyRecorder.svelte — press-to-record control for a command's hotkey -->
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onDestroy } from 'svelte';
	import { hotkeyFromEvent } from '$lib/utils/hotkey';

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
	 */
	async function resume(): Promise<void> {
		if (suspending) await suspending;
		await invoke('resume_hotkeys').catch((e) => (hint = String(e)));
	}

	function onkeydown(event: KeyboardEvent) {
		if (!recording) return;
		event.preventDefault();
		event.stopPropagation();
		if (event.key === 'Escape') {
			void stop(false);
			return;
		}
		const mapped = hotkeyFromEvent(event);
		if (mapped) {
			captured = mapped;
			hint = null;
			void stop(true);
		} else {
			hint = 'Hold a modifier (Ctrl, Alt, Shift or Win) and press a key.';
		}
	}

	// Backstop for the one exit path the handlers above cannot see: the
	// component being torn down mid-recording. The settings window is only
	// ever hidden, never destroyed, and Rust re-applies the config on that
	// hide as a defensive fallback (see the CloseRequested handler in
	// src-tauri/src/lib.rs) — but this component can still unmount inside a
	// live window, e.g. the surrounding CommandForm dialog closing (Cancel,
	// its own close button, or an Escape that a nested dismissable layer
	// consumes before this component's own keydown handler sees it). If that
	// happens, `resume()` still runs; it just cannot update `hint`, since
	// there is no UI left to show it on.
	onDestroy(() => {
		if (!recording) return;
		recording = false;
		void resume();
	});
</script>

<svelte:window on:keydown={recording ? onkeydown : undefined} />

<button
	{id}
	type="button"
	class="w-full rounded-md border px-3 py-2 text-left font-mono text-sm"
	class:ring-2={recording}
	onclick={start}
	onblur={() => void stop(false)}
>
	{recording ? 'Press a combination…' : value || 'Click to set a hotkey'}
</button>
{#if hint}
	<p class="mt-1 text-xs text-muted-foreground">{hint}</p>
{/if}
