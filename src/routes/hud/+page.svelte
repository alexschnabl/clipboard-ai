<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { fade } from 'svelte/transition';
	import { listen } from '@tauri-apps/api/event';
	import { invoke } from '@tauri-apps/api/core';
	import CheckIcon from '@lucide/svelte/icons/check';
	import XIcon from '@lucide/svelte/icons/x';
	import Keycaps from '$lib/components/Keycaps.svelte';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { applyOverlayLook, reasonSentence, stoppedTitle } from '$lib/overlay';
	import type { HudEvent } from '$lib/types';

	const INSET = 16;

	let current = $state<HudEvent | null>(null);
	let run = $state(0);
	let leaving = $state(false);
	let capsule = $state<HTMLElement | null>(null);

	async function report(id: number): Promise<void> {
		await tick();
		if (!capsule || current?.id !== id) return;
		await invoke('hud_measured', {
			id,
			width: capsule.offsetWidth + 2 * INSET,
			height: capsule.offsetHeight + 2 * INSET,
			inset: INSET
		});
	}

	function show(event: HudEvent): void {
		applyOverlayLook(event);
		if (event.fresh || !current) run++;
		current = event;
		leaving = false;
		void report(event.id);
	}

	onMount(() => {
		const unlistenView = listen<HudEvent>('hud://view', (e) => show(e.payload));
		const unlistenLeave = listen<number>('hud://leave', (e) => {
			if (current?.id === e.payload) leaving = true;
		});
		return () => {
			void unlistenView.then((f) => f());
			void unlistenLeave.then((f) => f());
		};
	});
</script>

<div class="hud-root" style:padding="{INSET}px" aria-live="polite">
	{#if current}
		{@const view = current.view}
		{#key run}
			<div bind:this={capsule} class="capsule" class:leaving role="status">
				{#key current.id}
					<div class="content" in:fade={{ duration: 120 }}>
						{#if view.kind === 'working'}
							<span class="bars" aria-hidden="true"><i></i><i></i><i></i></span>
							<span class="title">{view.command}</span>
							{#if view.backend}<span class="sub">{view.backend}</span>{/if}
							{#if view.cancellable}
								<span class="hint"><Keycaps hotkey="Esc" /><span class="sub">{i18n.t.hud.escHint}</span></span>
							{/if}
						{:else if view.kind === 'done'}
							<span class="badge ok" aria-hidden="true"><CheckIcon class="size-[11px]" strokeWidth={3} /></span>
							{#if view.again}
								<span class="title">{i18n.t.hud.pastedAgain}</span>
							{:else if view.sent}
								<span class="title">{i18n.t.hud.sent}</span>
							{:else}
								<span class="title">{i18n.t.hud.done}</span>
								{#if view.wordsChanged !== null}
									<span class="sub">
										{view.wordsChanged === 0
											? i18n.t.hud.noChanges
											: i18n.t.hud.wordsChanged(view.wordsChanged)}
									</span>
								{/if}
							{/if}
							{#if !view.sent}
								<span class="hint"><Keycaps hotkey="Ctrl+Z" /><span class="sub">{i18n.t.hud.undoHint}</span></span>
							{/if}
						{:else}
							<span
								class="badge"
								class:muted={view.reason === 'cancelled' || view.reason === 'no-last-result'}
								class:no={view.reason !== 'cancelled' && view.reason !== 'no-last-result'}
								aria-hidden="true"
							>
								<XIcon class="size-[11px]" strokeWidth={3} />
							</span>
							<span class="title">{stoppedTitle(view.reason, i18n.t)}</span>
							<span class="sub sentence">{reasonSentence(view.reason, i18n.t)}</span>
						{/if}
					</div>
				{/key}
			</div>
		{/key}
	{/if}
</div>

<style>
	.hud-root {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: flex-start;
		justify-content: flex-start;
		pointer-events: none;
	}

	.capsule {
		display: inline-flex;
		flex: none;
		align-items: center;
		height: 36px;
		padding: 0 14px 0 11px;
		border-radius: 999px;
		background: color-mix(in oklch, var(--popover) 97%, transparent);
		color: var(--popover-foreground);
		box-shadow:
			0 4px 14px -3px rgb(0 0 0 / 0.22),
			0 0 0 1px color-mix(in oklch, var(--foreground) 9%, transparent);
		font-size: 12.5px;
		white-space: nowrap;
		animation: capsule-in 260ms cubic-bezier(0.2, 1.3, 0.4, 1) both;
		transition:
			opacity 180ms ease,
			transform 180ms ease;
	}

	:global(.dark) .capsule {
		box-shadow:
			0 4px 14px -3px rgb(0 0 0 / 0.5),
			0 0 0 1px color-mix(in oklch, var(--foreground) 12%, transparent);
	}

	.capsule.leaving {
		opacity: 0;
		transform: translateY(3px) scale(0.98);
	}

	@keyframes capsule-in {
		from {
			opacity: 0;
			transform: translateY(6px) scale(0.94);
		}
	}

	.content {
		display: flex;
		align-items: center;
		gap: 9px;
	}

	.title {
		max-width: 260px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sub {
		color: var(--muted-foreground);
		max-width: 220px;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.sentence {
		max-width: 360px;
	}

	.hint {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		margin-left: 1px;
	}

	.hint :global(kbd) {
		padding: 2px 5px;
		font-size: 10.5px;
	}

	.badge {
		display: grid;
		place-items: center;
		width: 16px;
		height: 16px;
		flex: none;
		border-radius: 999px;
		color: white;
	}

	.badge.ok {
		background: var(--success);
	}

	.badge.no {
		background: var(--destructive);
	}

	.badge.muted {
		background: var(--muted-foreground);
	}

	:global(.dark) .badge {
		color: var(--background);
	}

	.bars {
		display: flex;
		align-items: center;
		gap: 2px;
		height: 14px;
		width: 13px;
		flex: none;
	}

	.bars i {
		width: 3px;
		border-radius: 2px;
		background: currentColor;
		opacity: 0.85;
		animation: bar 1s ease-in-out infinite;
	}

	.bars i:nth-child(1) {
		height: 6px;
	}

	.bars i:nth-child(2) {
		height: 12px;
		animation-delay: 0.15s;
	}

	.bars i:nth-child(3) {
		height: 8px;
		animation-delay: 0.3s;
	}

	@keyframes bar {
		50% {
			transform: scaleY(0.4);
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.capsule,
		.bars i {
			animation: none;
		}
	}
</style>
