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

	let suspending: Promise<void> | null = null;

	async function start() {
		if (recording) return;
		recording = true;
		captured = null;
		hint = null;
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

	async function resume(): Promise<void> {
		if (suspending) await suspending;
		try {
			configState.problems = await invoke<Problem[]>('resume_hotkeys');
		} catch (e) {
			hint = String(e);
		}
	}

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

	$effect(() => {
		if (!recording) return;
		document.addEventListener('keydown', onkeydown, { capture: true });
		return () => document.removeEventListener('keydown', onkeydown, { capture: true });
	});

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
