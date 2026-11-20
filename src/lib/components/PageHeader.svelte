<script lang="ts">
	import type { Snippet } from 'svelte';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import { i18n } from '$lib/i18n/i18n.svelte';

	let {
		title,
		subtitle,
		onBack,
		actions
	}: {
		title: string;
		subtitle?: string;
		onBack?: () => void;
		actions?: Snippet;
	} = $props();
</script>

<header class="flex shrink-0 items-center gap-2.5 px-7 pt-6 pb-4">
	{#if onBack}
		<button
			type="button"
			class="-ml-2 grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
			onclick={onBack}
			aria-label={i18n.t.common.back}
		>
			<ArrowLeftIcon class="size-4" />
		</button>
	{/if}
	<h1 class="truncate text-[19px] font-normal">{title}</h1>
	{#if subtitle}
		<span class="shrink-0 pt-0.5 text-[13px] text-muted-foreground">{subtitle}</span>
	{/if}
	<div class="ml-auto flex shrink-0 items-center gap-2">
		{@render actions?.()}
	</div>
</header>
