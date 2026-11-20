<script lang="ts">
	import '../app.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack, type Snippet } from 'svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { page } from '$app/state';

	let { children }: { children: Snippet } = $props();

	const overlay = $derived(page.route.id === '/hud' || page.route.id === '/palette');

	$effect(() => {
		document.documentElement.classList.toggle('overlay-window', overlay);
	});

	$effect(() => {
		if (overlay && configState.loading) return;
		const theme = configState.config.settings.theme;
		untrack(() => setMode(theme));
	});

	$effect(() => {
		const language = configState.config.settings.language;
		untrack(() => i18n.setLanguage(language));
	});

	const BLOCKED_KEYS = new Set(['F3', 'F5', 'F7']);
	const BLOCKED_CTRL_KEYS = new Set(['r', 'p', 'f', 'g', 'u', 's']);

	function onKeydown(event: KeyboardEvent): void {
		const ctrlKey = event.ctrlKey && !event.altKey && BLOCKED_CTRL_KEYS.has(event.key.toLowerCase());
		if (BLOCKED_KEYS.has(event.key) || ctrlKey) event.preventDefault();
	}

	function onContextMenu(event: MouseEvent): void {
		const target = event.target as HTMLElement | null;
		if (target?.closest('input, textarea, [contenteditable="true"]')) return;
		if (target?.closest('.select-text') && window.getSelection()?.toString()) return;
		event.preventDefault();
	}
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={onContextMenu} />

<ModeWatcher />
<main class={overlay ? 'text-foreground' : 'min-h-screen bg-background text-foreground'}>
	{@render children()}
</main>
