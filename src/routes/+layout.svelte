<script lang="ts">
	import '../app.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack, type Snippet } from 'svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';

	let { children }: { children: Snippet } = $props();

	$effect(() => {
		const theme = configState.config.settings.theme;
		untrack(() => setMode(theme));
	});

	$effect(() => {
		const language = configState.config.settings.language;
		untrack(() => i18n.setLanguage(language));
	});
</script>

<ModeWatcher />
<main class="min-h-screen bg-background text-foreground">
	{@render children()}
</main>
