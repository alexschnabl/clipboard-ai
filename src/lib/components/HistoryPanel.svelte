<!-- lib/components/HistoryPanel.svelte — last 100 cycle runs: view, expand to
     recover input/result text, copy the result, clear the whole log. -->
<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import * as Table from '$lib/components/shadcn/table/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import RiDeleteBinLine from 'remixicon-svelte/icons/delete-bin-line';
	import RiClipboardLine from 'remixicon-svelte/icons/clipboard-line';
	import RiCheckLine from 'remixicon-svelte/icons/check-line';
	import RiArrowRightSLine from 'remixicon-svelte/icons/arrow-right-s-line';
	import RiHistoryLine from 'remixicon-svelte/icons/history-line';
	import { configState } from '$lib/state/config.svelte.js';
	import { formatDuration, formatHistoryTimestamp } from '$lib/time-format';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { HistoryEntry } from '$lib/types';

	/** The retention cap history.rs enforces (`history::MAX_ENTRIES`) -- not
	 *  exported to the frontend anywhere, so mirrored here as a literal to
	 *  feed `i18n.t.history.hint`. Matches the brief ("last 100 runs"). */
	const HISTORY_LIMIT = 100;

	let confirmClearOpen = $state(false);
	/** Index into `configState.history` of the currently expanded row, or
	 *  `null` when none is. A single value, not a Set -- the mockup and the
	 *  density goal both point at "one row's detail open at a time", and a
	 *  single value is what makes "click the open row again to close it"
	 *  trivial below. */
	let expandedIndex = $state<number | null>(null);
	/** Which copy button last succeeded, keyed by `${rowKey}-input`/`-result`
	 *  so the transient "Copied" label can't bleed onto the wrong button when
	 *  two rows are copied in quick succession. */
	let copiedKey = $state<string | null>(null);
	let copyError = $state<string | null>(null);
	let copiedResetTimer: ReturnType<typeof setTimeout> | undefined;

	onMount(() => {
		void configState.loadHistory();
	});

	// F5 (review3-frontend.md): every other timer/listener in this codebase
	// (HotkeyRecorder's `onDestroy` backstop, +page.svelte's `unlisten`) is
	// cleared on unmount; this one wasn't. A
	// settings-window hide within 1.5s of "Copy" used to destroy this
	// component (it lives inside +page.svelte's `{#key configState.revision}`
	// block) while the timeout was still pending -- harmless (nothing reads
	// `copiedKey` on a dead instance), but inconsistent with the rest of the
	// app's cleanup discipline.
	onDestroy(() => {
		clearTimeout(copiedResetTimer);
	});

	// F3 (review3-frontend.md): `text-muted-foreground` on `bg-muted` is
	// 4.15:1 in light mode -- just under WCAG AA's 4.5:1 for this small
	// (10px), non-bold text. `text-foreground/65` is theme-aware for free
	// (`--foreground` itself flips between near-black and near-white), and
	// computed from the app's own oklch tokens it clears AA in both themes
	// with margin to spare against this same `bg-muted`: 6.03:1 light,
	// 6.88:1 dark (was 4.15:1 / 6.05:1).
	const headClass =
		'sticky top-0 z-10 h-7 bg-muted px-2 text-[10px] font-semibold tracking-wide text-foreground/65 uppercase';

	function toggleExpand(index: number): void {
		expandedIndex = expandedIndex === index ? null : index;
	}

	function outcomeMeta(outcome: HistoryEntry['outcome']): {
		label: string;
		dot: string;
		text: string;
	} {
		// Tokens only, per the brief: --primary for a real success (this
		// project's teal, not an invented green), --muted-foreground for an
		// abort (a cycle stopping itself on purpose is not an error), and
		// --destructive reserved for an actual backend failure.
		//
		// F2 (review3-frontend.md): plain `text-primary` on `bg-background`
		// computes to 2.62:1 in dark mode (well under AA's 4.5:1) -- dark
		// mode's --primary is a deliberately dark teal, calibrated against
		// `--primary-foreground` for use as a *background*, not as text on
		// `--background`. `dark:text-chart-2` overrides it only in dark mode
		// (light mode's plain `text-primary` already passes at 5.39:1) with
		// a brighter step from the same teal family already defined in
		// app.css's chart tokens -- computed at 8.14:1 against dark mode's
		// `--background`, comfortable margin above the 4.5:1 floor.
		switch (outcome.status) {
			case 'succeeded':
				return {
					label: i18n.t.history.statusSuccess,
					dot: 'bg-primary',
					text: 'text-primary dark:text-chart-2'
				};
			case 'aborted':
				return {
					label: i18n.t.history.statusAborted,
					dot: 'bg-muted-foreground',
					text: 'text-muted-foreground'
				};
			case 'failed':
				return {
					label: i18n.t.history.statusFailed,
					dot: 'bg-destructive',
					text: 'text-destructive'
				};
		}
	}

	/** `navigator.clipboard`, not `@tauri-apps/plugin-clipboard-manager` --
	 *  that plugin is not in package.json's dependencies (checked before
	 *  writing this), and the task brief says to reach for it only if it's
	 *  already a dependency. The Tauri webview exposes the standard Clipboard
	 *  API, so this needs nothing extra. */
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
		expandedIndex = null;
		await configState.clearHistory();
	}
</script>

<!-- Same content-toolbar / content-scroll split as CommandList.svelte -- see
     that component's comment for the spacing rationale. History's table
     previously sat in its own `max-h-[28rem]` box regardless of the window's
     actual height; it now fills whatever the toolbar leaves behind, matching
     the mockup's `.content-scroll { flex: 1 1 auto }` instead of an
     arbitrary cap. -->
<div class="flex h-full min-h-0 flex-col">
	<div class="flex shrink-0 items-center gap-2 border-b px-4 py-2.5">
		<h2 class="text-[12.5px] font-bold">{i18n.t.history.title}</h2>
		<span class="text-[11px] text-muted-foreground">{i18n.t.history.hint(HISTORY_LIMIT)}</span>
		<div class="flex-1"></div>
		<Button
			variant="outline"
			size="sm"
			onclick={() => (confirmClearOpen = true)}
			disabled={configState.history.length === 0}
		>
			<RiDeleteBinLine class="size-4" />
			{i18n.t.history.clear}
		</Button>
	</div>

	<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
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

		{#if configState.historyLoading}
			<p class="text-sm text-muted-foreground">{i18n.t.app.loading}</p>
		{:else if configState.history.length === 0}
			<div class="flex flex-col items-center gap-1 py-12 text-center">
				<RiHistoryLine class="size-6 text-muted-foreground opacity-50" />
				<p class="text-sm font-medium">{i18n.t.history.emptyTitle}</p>
				<p class="max-w-64 text-xs text-muted-foreground">{i18n.t.history.emptyDescription}</p>
			</div>
		{:else}
			<Table.Root class="text-xs">
				<Table.Header>
					<Table.Row class="hover:bg-transparent">
						<Table.Head class="{headClass} w-20">{i18n.t.history.columnTime}</Table.Head>
						<Table.Head class={headClass}>{i18n.t.history.columnCommand}</Table.Head>
						<Table.Head class={headClass}>{i18n.t.history.columnBackend}</Table.Head>
						<Table.Head class="{headClass} w-24">{i18n.t.history.columnOutcome}</Table.Head>
						<Table.Head class="{headClass} w-20 text-right"
							>{i18n.t.history.columnDuration}</Table.Head
						>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each configState.history as entry, i (`${entry.timestampMs}-${entry.commandId}-${i}`)}
						{@const meta = outcomeMeta(entry.outcome)}
						{@const rowKey = `${entry.timestampMs}-${entry.commandId}-${i}`}
						{@const expanded = expandedIndex === i}
						<Table.Row>
							<Table.Cell class="py-1.5 font-mono text-[11px] text-muted-foreground">
								{formatHistoryTimestamp(entry.timestampMs)}
							</Table.Cell>
							<Table.Cell class="max-w-48 py-1.5 font-medium">
								<button
									type="button"
									class="flex w-full items-center gap-1 text-left"
									onclick={() => toggleExpand(i)}
									aria-expanded={expanded}
									aria-label={expanded
										? i18n.t.history.collapseAria(entry.commandName)
										: i18n.t.history.expandAria(entry.commandName)}
								>
									<RiArrowRightSLine
										class="size-3.5 shrink-0 text-muted-foreground transition-transform {expanded
											? 'rotate-90'
											: ''}"
									/>
									<span class="truncate" title={entry.commandName}>{entry.commandName}</span>
								</button>
							</Table.Cell>
							<Table.Cell class="max-w-40 py-1.5">
								<span class="block truncate text-muted-foreground" title={entry.backendName}
									>{entry.backendName}</span
								>
							</Table.Cell>
							<Table.Cell class="py-1.5">
								<span class="inline-flex items-center gap-1.5 text-xs font-medium {meta.text}">
									<span class="size-1.5 shrink-0 rounded-full {meta.dot}"></span>
									{meta.label}
								</span>
							</Table.Cell>
							<Table.Cell class="py-1.5 text-right font-mono text-[11px] text-muted-foreground">
								{formatDuration(entry.totalCycleMs ?? entry.backendMs)}
							</Table.Cell>
						</Table.Row>
						{#if expanded}
							<Table.Row class="hover:bg-transparent">
								<Table.Cell colspan={5} class="whitespace-normal bg-muted/30 p-3">
									<div class="flex flex-col gap-3">
										{#if entry.outcome.status === 'aborted'}
											<p class="text-xs text-muted-foreground">
												{i18n.t.history.abortedPrefix}
												{entry.outcome.reason}
											</p>
										{:else if entry.outcome.status === 'failed'}
											<p class="text-xs text-destructive">
												{i18n.t.history.failedPrefix}
												{entry.outcome.error}
											</p>
										{/if}

										<div class="grid grid-cols-1 gap-3 md:grid-cols-2">
											<div class="flex flex-col gap-1">
												<div class="flex items-center justify-between">
													<span class="text-xs font-medium text-muted-foreground"
														>{i18n.t.history.inputLabel}</span
													>
													{#if entry.inputText}
														<Button
															variant="ghost"
															size="xs"
															onclick={() =>
																void copyText(entry.inputText ?? '', `${rowKey}-input`)}
														>
															{#if copiedKey === `${rowKey}-input`}
																<RiCheckLine class="size-3.5" />
																{i18n.t.history.copied}
															{:else}
																<RiClipboardLine class="size-3.5" />
																{i18n.t.history.copy}
															{/if}
														</Button>
													{/if}
												</div>
												<pre
													class="max-h-40 overflow-auto rounded-md border bg-background p-2 text-[11px] whitespace-pre-wrap">{entry.inputText ??
														'— nothing was captured for this run —'}</pre>
											</div>

											<div class="flex flex-col gap-1">
												<div class="flex items-center justify-between">
													<span class="text-xs font-medium text-muted-foreground"
														>{i18n.t.history.resultLabel}</span
													>
													{#if entry.resultText}
														<Button
															variant="outline"
															size="xs"
															onclick={() =>
																void copyText(entry.resultText ?? '', `${rowKey}-result`)}
														>
															{#if copiedKey === `${rowKey}-result`}
																<RiCheckLine class="size-3.5" />
																{i18n.t.history.copied}
															{:else}
																<RiClipboardLine class="size-3.5" />
																{i18n.t.history.copyResult}
															{/if}
														</Button>
													{/if}
												</div>
												<pre
													class="max-h-40 overflow-auto rounded-md border bg-background p-2 text-[11px] whitespace-pre-wrap">{entry.resultText ??
														'— nothing was pasted for this run —'}</pre>
											</div>
										</div>
									</div>
								</Table.Cell>
							</Table.Row>
						{/if}
					{/each}
				</Table.Body>
			</Table.Root>
		{/if}
	</div>
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
