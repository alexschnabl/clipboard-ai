<script lang="ts">
	import AppWindowIcon from '@lucide/svelte/icons/app-window';
	import { runningApps } from '$lib/state/apps.svelte.js';

	let { exe, class: className = 'size-5' }: { exe: string; class?: string } = $props();

	const src = $derived(runningApps.iconFor(exe));
</script>

{#if src}
	<img {src} alt="" class={['shrink-0 object-contain', className]} draggable="false" />
{:else}
	<span
		class={['grid shrink-0 place-items-center rounded-[6px] bg-muted text-muted-foreground', className]}
		aria-hidden="true"
	>
		<AppWindowIcon class="size-[65%]" />
	</span>
{/if}
