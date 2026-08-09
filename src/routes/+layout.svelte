<script lang="ts">
	import '../app.css';
	import { ModeWatcher, setMode } from 'mode-watcher';
	import { untrack, type Snippet } from 'svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';

	let { children }: { children: Snippet } = $props();

	// Theme is persisted at `configState.config.settings.theme`
	// ('system' | 'light' | 'dark' -- src-tauri/src/config.rs `Settings`/`Theme`).
	// The Settings section owns *writing* that value; this effect is the one place
	// that makes `<ModeWatcher />` *follow* it, so there's a single source of truth
	// for which theme renders, not a second one living in this layout.
	//
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

	// The counterpart to the theme effect above, and it was simply missing:
	// `i18n.setLanguage` had no caller anywhere in the app, so the store sat
	// on its `'system'` default forever and choosing a language persisted to
	// config.json without ever changing a word on screen. The i18n module's
	// own doc comment specifies this caller ("typically once after
	// `configState.load()` resolves, and again whenever the user changes the
	// setting"); it just never got written, because the module and the
	// settings UI were built separately and each assumed the other end
	// existed.
	//
	// Driving it from the persisted value covers both cases at once: `load()`
	// replaces `config`, and the settings control writes through `commit()`,
	// so this one effect handles startup and every later change.
	//
	// `untrack` around the call for the same reason as the theme effect --
	// `setLanguage` writes reactive state, and only the read above should be
	// this effect's dependency.
	$effect(() => {
		const language = configState.config.settings.language;
		untrack(() => i18n.setLanguage(language));
	});
</script>

<ModeWatcher />
<main class="min-h-screen bg-background text-foreground">
	{@render children()}
</main>
