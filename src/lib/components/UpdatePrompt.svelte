<script lang="ts">
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { fly } from 'svelte/transition';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { updateState } from '$lib/state/updates.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';

	const t = $derived(i18n.t.update);
	const visible = $derived(
		updateState.available !== null &&
			((updateState.phase === 'available' && !updateState.promptDismissed) ||
				updateState.phase === 'installing')
	);
</script>

{#if visible && updateState.available}
	<div
		class="pointer-events-auto absolute right-4 bottom-4 z-20 w-80 rounded-[14px] border bg-popover p-4 text-popover-foreground shadow-[0_8px_24px_rgb(0_0_0/0.12)]"
		role="status"
		aria-live="polite"
		transition:fly={{ y: 8, duration: 160 }}
	>
		<div class="flex items-start gap-3">
			<span class="grid size-8 shrink-0 place-items-center rounded-[9px] border bg-canvas text-muted-foreground">
				<DownloadIcon class="size-4" />
			</span>
			<div class="min-w-0 flex-1">
				<p class="text-[14px]">{t.promptTitle(updateState.available.version)}</p>
				<p class="mt-0.5 text-[13px] text-muted-foreground">
					{#if updateState.phase === 'installing'}
						{updateState.percent === null ? t.downloading : t.downloadingPercent(updateState.percent)}
					{:else if updateState.installError}
						<span class="text-destructive" title={updateState.installError}
							>{i18n.t.settings.updateInstallFailed}</span
						>
					{:else}
						{t.promptText}
					{/if}
				</p>
			</div>
		</div>
		{#if updateState.phase !== 'installing'}
			<div class="mt-3 flex justify-end gap-2">
				<Button variant="ghost" size="sm" onclick={() => (updateState.promptDismissed = true)}
					>{t.later}</Button
				>
				<Button size="sm" onclick={() => void updateState.install()}>{t.install}</Button>
			</div>
		{/if}
	</div>
{/if}
