<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { invoke } from '@tauri-apps/api/core';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import CommandIcon from '@lucide/svelte/icons/command';
	import Keycaps from '$lib/components/Keycaps.svelte';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { applyOverlayLook, nextIndex, paletteRows, type PaletteRow } from '$lib/overlay';
	import type { PaletteChoice, PaletteOpenEvent } from '$lib/types';

	const INSET = 20;

	let session = $state<PaletteOpenEvent | null>(null);
	let query = $state('');
	let selected = $state(0);
	let backendId = $state<string | null>(null);
	let armed = $state(false);
	let sent = $state(false);
	let card = $state<HTMLElement | null>(null);
	let input = $state<HTMLInputElement | null>(null);
	let list = $state<HTMLElement | null>(null);

	const backends = $derived(session?.backends ?? []);
	const backendName = $derived(backends.find((b) => b.id === backendId)?.name ?? null);
	const canInstruct = $derived(backendId !== null && backends.length > 0);
	const rows = $derived(session ? paletteRows(session.commands, query, canInstruct) : []);
	const hasCommands = $derived(rows.some((r) => r.kind === 'command'));
	const firstInstruction = $derived(rows.findIndex((r) => r.kind === 'instruction'));

	$effect(() => {
		if (selected >= rows.length) selected = Math.max(0, rows.length - 1);
	});

	$effect(() => {
		const id = session?.session;
		void rows.length;
		if (id !== undefined) void report(id);
	});

	async function report(id: number): Promise<void> {
		await tick();
		if (!card || session?.session !== id) return;
		await invoke('palette_measured', {
			session: id,
			width: card.offsetWidth + 2 * INSET,
			height: card.offsetHeight + 2 * INSET,
			inset: INSET
		});
	}

	async function open(event: PaletteOpenEvent): Promise<void> {
		applyOverlayLook(event);
		query = '';
		selected = 0;
		armed = false;
		sent = false;
		backendId = event.backendId;
		session = event;
		await tick();
		input?.focus();
	}

	function choose(row: PaletteRow | undefined): void {
		if (!session || !row || sent) return;
		let choice: PaletteChoice;
		if (row.kind === 'command') {
			choice = { kind: 'command', commandId: row.command.id };
		} else {
			if (!backendId) return;
			choice = { kind: 'instruction', text: row.text, backendId };
		}
		sent = true;
		void invoke('palette_run', { session: session.session, choice });
	}

	function close(): void {
		if (!session || sent) return;
		sent = true;
		void invoke('palette_close', { session: session.session });
	}

	function cycleBackend(delta: number): void {
		if (backends.length === 0) return;
		const current = backends.findIndex((b) => b.id === backendId);
		const next = backends[nextIndex(Math.max(current, 0), current < 0 ? 0 : delta, backends.length)];
		if (next) backendId = next.id;
	}

	async function move(delta: number): Promise<void> {
		selected = nextIndex(selected, delta, rows.length);
		await tick();
		list?.querySelector(`[data-index="${selected}"]`)?.scrollIntoView({ block: 'nearest' });
	}

	function onkeydown(event: KeyboardEvent): void {
		if (!session || event.isComposing) return;
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				void move(1);
				break;
			case 'ArrowUp':
				event.preventDefault();
				void move(-1);
				break;
			case 'Tab':
				event.preventDefault();
				cycleBackend(event.shiftKey ? -1 : 1);
				break;
			case 'Escape':
				event.preventDefault();
				close();
				break;
			case 'Enter':
				event.preventDefault();
				armed = true;
				break;
		}
	}

	function onkeyup(event: KeyboardEvent): void {
		if (event.key !== 'Enter' || !armed) return;
		event.preventDefault();
		armed = false;
		choose(rows[selected]);
	}

	function oninput(): void {
		selected = 0;
	}

	onMount(() => {
		const unlisten = listen<PaletteOpenEvent>('palette://open', (e) => void open(e.payload));
		const refocus = (): void => input?.focus();
		window.addEventListener('focus', refocus);
		return () => {
			void unlisten.then((f) => f());
			window.removeEventListener('focus', refocus);
		};
	});
</script>

<svelte:window {onkeydown} {onkeyup} />

<div class="palette-root" style:padding="{INSET}px">
	{#if session}
		{#key session.session}
			<div bind:this={card} class="card">
				<div class="search">
					{#if query.trim() && !hasCommands}
						<SparklesIcon class="size-4 shrink-0 text-muted-foreground" />
					{:else}
						<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
					{/if}
					<input
						bind:this={input}
						bind:value={query}
						{oninput}
						type="text"
						role="combobox"
						aria-expanded="true"
						spellcheck="false"
						autocomplete="off"
						placeholder={i18n.t.palette.placeholder}
						aria-label={i18n.t.palette.placeholder}
						aria-controls="palette-list"
						aria-activedescendant={rows.length > 0 ? `palette-row-${selected}` : undefined}
					/>
				</div>

				<div
					bind:this={list}
					id="palette-list"
					class="list"
					role="listbox"
					aria-label={i18n.t.palette.listLabel}
				>
					{#if hasCommands}
						<div class="group">{i18n.t.palette.groupCommands}</div>
					{/if}
					{#each rows as row, i (row.kind === 'command' ? row.command.id : '__instruction')}
						{#if row.kind === 'instruction' && i === firstInstruction && hasCommands}
							<div class="divider" role="presentation"></div>
						{/if}
						<div
							id="palette-row-{i}"
							data-index={i}
							class="row"
							class:on={i === selected}
							role="option"
							tabindex="-1"
							aria-selected={i === selected}
							onmousemove={() => (selected = i)}
							onclick={() => choose(row)}
							onkeydown={() => {}}
						>
							{#if row.kind === 'command'}
								<CommandIcon class="size-[15px] shrink-0 text-muted-foreground" />
								<span class="name">{row.command.name}</span>
								{#if row.command.backendName}
									<span class="meta">{row.command.backendName}</span>
								{/if}
								{#if row.command.hotkey}
									<span class="keys"><Keycaps hotkey={row.command.hotkey} /></span>
								{/if}
							{:else}
								<SparklesIcon class="size-[15px] shrink-0 text-muted-foreground" />
								<span class="name">{i18n.t.palette.runInstruction(row.text)}</span>
								<span class="meta">{i18n.t.palette.instructionHint}</span>
							{/if}
						</div>
					{/each}

					{#if rows.length === 0}
						<p class="empty">
							{#if query.trim() && backends.length === 0}
								{i18n.t.palette.noBackend}
							{:else}
								{i18n.t.palette.empty}
							{/if}
						</p>
					{:else if query.trim() && !hasCommands && session.commands.length > 0}
						<p class="empty">{i18n.t.palette.noMatches}</p>
					{/if}
				</div>

				<div class="foot">
					<span class="hint"><Keycaps hotkey="Enter" />{i18n.t.palette.footerRun}</span>
					{#if backendName}
						<span class="hint">
							<Keycaps hotkey="Tab" />{i18n.t.palette.footerBackend}
							<span class="backend">{backendName}</span>
						</span>
					{/if}
					<span class="hint ml-auto"><Keycaps hotkey="Esc" />{i18n.t.palette.footerClose}</span>
				</div>
			</div>
		{/key}
	{/if}
</div>

<style>
	.palette-root {
		position: fixed;
		inset: 0;
		display: flex;
		align-items: flex-start;
	}

	.card {
		width: 440px;
		flex: none;
		overflow: hidden;
		border-radius: 14px;
		background: color-mix(in oklch, var(--popover) 98%, transparent);
		color: var(--popover-foreground);
		box-shadow:
			0 8px 18px -6px rgb(0 0 0 / 0.28),
			0 0 0 1px color-mix(in oklch, var(--foreground) 9%, transparent);
		font-size: 13px;
		animation: card-in 220ms cubic-bezier(0.2, 1.2, 0.4, 1) both;
	}

	:global(.dark) .card {
		box-shadow:
			0 8px 18px -6px rgb(0 0 0 / 0.55),
			0 0 0 1px color-mix(in oklch, var(--foreground) 12%, transparent);
	}

	@keyframes card-in {
		from {
			opacity: 0;
			transform: translateY(4px) scale(0.97);
		}
	}

	.search {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 12px 14px;
		border-bottom: 1px solid var(--border);
	}

	.search input {
		flex: 1;
		min-width: 0;
		background: transparent;
		border: none;
		outline: none;
		font-size: 14px;
		color: inherit;
	}

	.search input::placeholder {
		color: var(--muted-foreground);
	}

	.list {
		max-height: 312px;
		overflow-y: auto;
		padding: 4px 6px 6px;
	}

	.group {
		padding: 8px 8px 4px;
		font-size: 11.5px;
		color: var(--muted-foreground);
	}

	.divider {
		height: 1px;
		margin: 5px 8px;
		background: var(--border);
	}

	.row {
		display: flex;
		align-items: center;
		gap: 10px;
		height: 36px;
		padding: 0 8px;
		border-radius: 8px;
		cursor: default;
		transition: background-color 90ms ease;
	}

	.row.on {
		background: var(--accent);
	}

	.name {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.meta {
		flex: none;
		max-width: 140px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		font-size: 12px;
		color: var(--muted-foreground);
	}

	.keys {
		margin-left: auto;
		flex: none;
	}

	.row .meta:last-child {
		margin-left: auto;
	}

	.empty {
		padding: 10px 8px 6px;
		font-size: 12.5px;
		color: var(--muted-foreground);
	}

	.foot {
		display: flex;
		align-items: center;
		gap: 14px;
		padding: 8px 14px;
		border-top: 1px solid var(--border);
		font-size: 11.5px;
		color: var(--muted-foreground);
	}

	.hint {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		white-space: nowrap;
	}

	.hint :global(kbd),
	.keys :global(kbd) {
		padding: 2px 5px;
		font-size: 10.5px;
	}

	.backend {
		max-width: 150px;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--popover-foreground);
	}

	@media (prefers-reduced-motion: reduce) {
		.card {
			animation: none;
		}
	}
</style>
