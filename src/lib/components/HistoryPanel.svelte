<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import CheckIcon from '@lucide/svelte/icons/check';
	import HistoryIcon from '@lucide/svelte/icons/rotate-ccw-clock';
	import SearchIcon from '@lucide/svelte/icons/search';
	import PageHeader from './PageHeader.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { formatDuration, formatHistoryTimestamp } from '$lib/time-format';
	import { wordDiff } from '$lib/diff';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { HistoryEntry } from '$lib/types';

	const HISTORY_LIMIT = 100;

	let confirmClearOpen = $state(false);
	let selectedKey = $state<string | null>(null);
	let query = $state('');
	let copiedKey = $state<string | null>(null);
	let copyError = $state<string | null>(null);
	let copiedResetTimer: ReturnType<typeof setTimeout> | undefined;

	onMount(() => {
		void configState.loadHistory();
	});

	onDestroy(() => {
		clearTimeout(copiedResetTimer);
	});

	const rows = $derived(
		configState.history.map((entry, i) => ({
			entry,
			key: `${entry.timestampMs}-${entry.commandId}-${i}`
		}))
	);

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		if (!q) return rows;
		return rows.filter(({ entry }) =>
			[entry.commandName, entry.backendName, entry.inputText, entry.resultText].some((v) =>
				v?.toLowerCase().includes(q)
			)
		);
	});

	const selected = $derived(filtered.find((r) => r.key === selectedKey) ?? filtered[0] ?? null);

	const changes = $derived.by(() => {
		const e = selected?.entry;
		if (!e || e.outcome.status !== 'succeeded' || !e.inputText || !e.resultText) return null;
		return wordDiff(e.inputText, e.resultText);
	});

	function outcomeMeta(outcome: HistoryEntry['outcome']): { label: string; dot: string } {
		switch (outcome.status) {
			case 'succeeded':
				return { label: i18n.t.history.statusSuccess, dot: 'bg-success' };
			case 'aborted':
				return { label: i18n.t.history.statusAborted, dot: 'bg-warning' };
			case 'failed':
				return { label: i18n.t.history.statusFailed, dot: 'bg-destructive' };
		}
	}

	function preview(entry: HistoryEntry): string {
		return (entry.resultText ?? entry.inputText ?? entry.commandName).replace(/\s+/g, ' ').trim();
	}

	function fullTimestamp(ms: number): string {
		return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(
			new Date(ms)
		);
	}

	function durationText(entry: HistoryEntry): string {
		if (entry.totalCycleMs == null) return formatDuration(entry.backendMs);
		return i18n.t.history.durationDetail(
			formatDuration(entry.totalCycleMs),
			formatDuration(entry.backendMs)
		);
	}

	async function copyText(text: string, key: string): Promise<void> {
		copyError = null;
		try {
			await navigator.clipboard.writeText(text);
			copiedKey = key;
			clearTimeout(copiedResetTimer);
			copiedResetTimer = setTimeout(() => {
				copiedKey = null;
			}, 1500);
		} catch (e) {
			copyError = String(e);
		}
	}

	async function confirmClear(): Promise<void> {
		confirmClearOpen = false;
		selectedKey = null;
		await configState.clearHistory();
	}
</script>

{#snippet copyButton(text: string | null, key: string, label: string)}
	{#if text}
		<Button variant="ghost" size="xs" onclick={() => void copyText(text, key)}>
			{#if copiedKey === key}
				<CheckIcon class="size-3.5" />
				{i18n.t.history.copied}
			{:else}
				<CopyIcon class="size-3.5" />
				{label}
			{/if}
		</Button>
	{/if}
{/snippet}

<div class="flex h-full min-h-0 flex-col">
	<PageHeader title={i18n.t.history.title} subtitle={i18n.t.history.hint(HISTORY_LIMIT)}>
		{#snippet actions()}
			<Button
				variant="ghost"
				size="sm"
				onclick={() => (confirmClearOpen = true)}
				disabled={configState.history.length === 0}
			>
				{i18n.t.history.clear}
			</Button>
		{/snippet}
	</PageHeader>

	{#if configState.historyError || copyError}
		<div class="flex flex-col gap-2 px-7 pb-3">
			{#if configState.historyError}
				<Alert.Root variant="destructive">
					<Alert.Title>{i18n.t.history.loadErrorTitle}</Alert.Title>
					<Alert.Description>{configState.historyError}</Alert.Description>
				</Alert.Root>
			{/if}
			{#if copyError}
				<Alert.Root variant="destructive">
					<Alert.Title>{i18n.t.history.copyErrorTitle}</Alert.Title>
					<Alert.Description>{copyError}</Alert.Description>
				</Alert.Root>
			{/if}
		</div>
	{/if}

	{#if configState.historyLoading}
		<p class="px-7 text-sm text-muted-foreground">{i18n.t.app.loading}</p>
	{:else if configState.history.length === 0}
		<div class="flex flex-col items-center gap-1 py-16 text-center">
			<HistoryIcon class="size-6 text-muted-foreground opacity-50" />
			<p class="text-sm font-medium">{i18n.t.history.emptyTitle}</p>
			<p class="max-w-64 text-xs text-muted-foreground">{i18n.t.history.emptyDescription}</p>
		</div>
	{:else}
		<div class="flex min-h-0 flex-1 gap-4 px-7 pb-7">
			<div class="flex w-[290px] shrink-0 flex-col gap-2">
				<label
					class="flex h-9 items-center gap-2 rounded-[9px] border bg-card px-2.5 text-muted-foreground focus-within:border-ring"
				>
					<SearchIcon class="size-4 shrink-0" />
					<input
						type="search"
						bind:value={query}
						placeholder={i18n.t.history.searchPlaceholder}
						class="min-w-0 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
					/>
				</label>
				<div class="-mx-1 flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-1">
					{#each filtered as row (row.key)}
						{@const meta = outcomeMeta(row.entry.outcome)}
						<button
							type="button"
							class={[
								'flex flex-col gap-1 rounded-[10px] px-3 py-2.5 text-left transition-colors duration-150',
								selected?.key === row.key ? 'bg-accent' : 'hover:bg-canvas'
							]}
							aria-current={selected?.key === row.key ? 'true' : undefined}
							onclick={() => (selectedKey = row.key)}
						>
							<span class="flex items-baseline gap-2">
								<span class="min-w-0 flex-1 truncate text-[14px]">{preview(row.entry)}</span>
								<span class="shrink-0 text-[11.5px] text-muted-foreground/80"
									>{formatHistoryTimestamp(row.entry.timestampMs)}</span
								>
							</span>
							<span class="flex items-center gap-1.5 truncate text-xs text-muted-foreground">
								{#if row.entry.outcome.status !== 'succeeded'}
									<span class={['size-1.5 shrink-0 rounded-full', meta.dot]}></span>
									<span class="shrink-0">{meta.label} -</span>
								{/if}
								<span class="truncate">{row.entry.commandName}</span>
							</span>
						</button>
					{:else}
						<p class="px-3 py-6 text-center text-xs text-muted-foreground">
							{i18n.t.history.noMatches}
						</p>
					{/each}
				</div>
			</div>

			{#if selected}
				{@const entry = selected.entry}
				{@const meta = outcomeMeta(entry.outcome)}
				<section class="list-card flex min-w-0 flex-1 flex-col gap-5 overflow-y-auto p-5 select-text">
					<div>
						<div class="text-[16px]">{entry.commandName}</div>
						<div class="mt-0.5 text-xs text-muted-foreground">{fullTimestamp(entry.timestampMs)}</div>
					</div>

					{#if entry.outcome.status !== 'succeeded'}
						<div class="flex items-start gap-2.5 rounded-[10px] border px-3 py-2.5 text-[13px]">
							<span class={['mt-1.5 size-2 shrink-0 rounded-full', meta.dot]}></span>
							<span>
								{entry.outcome.status === 'aborted'
									? `${i18n.t.history.abortedPrefix} ${entry.outcome.reason}`
									: `${i18n.t.history.failedPrefix} ${entry.outcome.error}`}
							</span>
						</div>
					{/if}

					{#if changes}
						<div class="flex flex-col gap-1.5">
							<div class="flex items-center justify-between">
								<span class="text-xs text-muted-foreground">{i18n.t.history.changesLabel}</span>
								{@render copyButton(entry.resultText, `${selected.key}-result`, i18n.t.history.copyResult)}
							</div>
							<p class="text-[13.5px] leading-relaxed whitespace-pre-wrap">
								{#each changes as part, i (i)}
									{#if part.kind === 'same'}{part.text}{:else if part.kind === 'added'}<ins
											class="rounded-[3px] bg-success/15 px-0.5 no-underline">{part.text}</ins
										>{:else}<del
											class="rounded-[3px] bg-destructive/12 px-0.5 text-muted-foreground decoration-destructive/60"
											>{part.text}</del
										>{/if}
								{/each}
							</p>
						</div>
					{:else}
						<div class="flex flex-col gap-1.5">
							<div class="flex items-center justify-between">
								<span class="text-xs text-muted-foreground">{i18n.t.history.resultLabel}</span>
								{@render copyButton(entry.resultText, `${selected.key}-result`, i18n.t.history.copyResult)}
							</div>
							<p class="max-h-56 overflow-y-auto text-[13.5px] leading-relaxed whitespace-pre-wrap">
								{entry.resultText ?? i18n.t.history.noResultPasted}
							</p>
						</div>
					{/if}

					<div class="flex flex-col gap-1.5">
						<div class="flex items-center justify-between">
							<span class="text-xs text-muted-foreground">{i18n.t.history.inputLabel}</span>
							{@render copyButton(entry.inputText, `${selected.key}-input`, i18n.t.history.copy)}
						</div>
						<p
							class="max-h-40 overflow-y-auto text-[13px] leading-relaxed whitespace-pre-wrap text-muted-foreground"
						>
							{entry.inputText ?? i18n.t.history.noInputCaptured}
						</p>
					</div>

					<dl class="grid grid-cols-[120px_1fr] text-[13px]">
						<dt class="py-2 text-muted-foreground">{i18n.t.history.columnBackend}</dt>
						<dd class="py-2">{entry.backendName}</dd>
						<dt class="border-t py-2 text-muted-foreground">{i18n.t.history.columnOutcome}</dt>
						<dd class="border-t py-2">{meta.label}</dd>
						<dt class="border-t py-2 text-muted-foreground">{i18n.t.history.columnDuration}</dt>
						<dd class="border-t py-2">{durationText(entry)}</dd>
					</dl>
				</section>
			{/if}
		</div>
	{/if}
</div>

<Dialog.Root bind:open={confirmClearOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{i18n.t.history.clearTitle}</Dialog.Title>
			<Dialog.Description>
				{i18n.t.history.clearDescription(configState.history.length)}
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (confirmClearOpen = false)}
				>{i18n.t.common.cancel}</Button
			>
			<Button variant="destructive" onclick={() => void confirmClear()}
				>{i18n.t.history.clear}</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
