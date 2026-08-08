<script lang="ts">
	import '../app.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack, type Snippet } from 'svelte';
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
	// The read is deliberately outside `untrack` and the write inside it.
	//
	// `setMode` assigns to mode-watcher's own `userPrefersMode.current`, whose
	// backing store both reads and writes reactive state. An `$effect` tracks
	// every reactive read that happens *while it runs*, including ones inside
	// a function it calls -- so calling `setMode` bare made this effect depend
	// on the very state `setMode` writes. It re-triggered itself until Svelte
	// aborted the batch with `effect_update_depth_exceeded`, and because that
	// kills the whole update batch, `configState.load()`'s result never
	// rendered: the window sat on "Loading…" forever.
	//
	// Reading `theme` first keeps the one dependency this effect actually
	// wants (re-run when the persisted theme changes); untracking the call
	// keeps mode-watcher's internals out of that dependency set.
	$effect(() => {
		const theme = configState.config.settings.theme;
		untrack(() => setMode(theme));
	});
</script>

<ModeWatcher />
<main class="min-h-screen bg-background text-foreground">
	{@render children()}
</main>
