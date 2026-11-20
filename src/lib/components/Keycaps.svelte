<script lang="ts">
	let { hotkey }: { hotkey: string } = $props();

	const parts = $derived(hotkey.split('+').filter((p) => p.length > 0));
</script>

<span class="inline-flex shrink-0 items-center gap-[3px]">
	{#each parts as part, i (i)}
		<kbd
			class="rounded-[5px] border border-b-2 bg-muted px-1.5 py-[3px] font-mono text-[11px] leading-none text-foreground"
			>{part}</kbd
		>
	{/each}
</span>
