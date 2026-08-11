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
