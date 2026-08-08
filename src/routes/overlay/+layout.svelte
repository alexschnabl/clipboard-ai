<!--
	routes/overlay/+layout.svelte — strips the settings window's chrome back
	out for this window only.

	The root layout (src/routes/+layout.svelte, shared with the settings
	window, not ours to edit) wraps every route in `<main class="min-h-screen
	bg-background text-foreground">`. That's correct for a document window and
	wrong for this one: the overlay window is declared `transparent: true` in
	tauri.conf.json, and Tauri/wry only makes the WebView2 *controller*
	background transparent (confirmed against wry 0.55.1's
	`webview2/mod.rs::set_background_color`) — anything the page itself still
	paints an opaque color over shows up as a solid box floating next to the
	cursor. `:global` is safe to reach for here specifically because each
	Tauri window loads its own independent HTML document: this stylesheet is
	compiled into the overlay window's bundle and can never be loaded by the
	settings window's.
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<style>
	:global(html),
	:global(body),
	:global(main) {
		background: transparent !important;
	}
</style>

{@render children()}
