<script lang="ts">
	import '../app.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import type { Snippet } from 'svelte';
	import { configState } from '$lib/state/config.svelte.js';

	let { children }: { children: Snippet } = $props();

	// Theme is persisted at `configState.config.settings.theme`
	// ('system' | 'light' | 'dark' -- src-tauri/src/config.rs `Settings`/`Theme`).
	// The Settings section owns *writing* that value; this effect is the one place
	// that makes `<ModeWatcher />` *follow* it, so there's a single source of truth
	// for which theme renders, not a second one living in this layout.
	//
	// This layout is shared with the overlay window's route (src/routes/overlay/,
	// not ours to touch) via the same component tree, so this runs there too --
	// harmlessly: that window's own `configState` instance never calls `load()`,
	// stays on the `'system'` fallback below, which is exactly what an unwired
	// `<ModeWatcher />` already defaulted to before this effect existed.
	$effect(() => {
		setMode(configState.config.settings.theme);
	});
</script>

<ModeWatcher />
<main class="min-h-screen bg-background text-foreground">
	{@render children()}
</main>
