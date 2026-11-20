<script lang="ts">
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import XIcon from '@lucide/svelte/icons/x';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import TrashIcon from '@lucide/svelte/icons/trash';
	import PageHeader from './PageHeader.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { addKeepWords, upsertReplacement } from '$lib/editor';
	import type { Vocabulary } from '$lib/types';

	let newWord = $state('');
	let editing = $state<number | 'new' | null>(null);
	let fromValue = $state('');
	let toValue = $state('');
	let saveError = $state<string | null>(null);
	let fromInput = $state<HTMLInputElement | null>(null);

	const vocabulary = $derived(configState.vocabulary);

	async function commit(next: Vocabulary): Promise<boolean> {
		saveError = null;
		const ok = await configState.updateVocabulary(next);
		if (!ok) {
			saveError = configState.error?.message ?? i18n.t.vocabulary.saveFailedFallback;
			configState.error = null;
		}
		return ok;
	}

	async function addWord(): Promise<void> {
		const keep = addKeepWords(vocabulary.keep, newWord);
		if (keep.length === vocabulary.keep.length) {
			newWord = '';
			return;
		}
		const ok = await commit({ ...$state.snapshot(vocabulary), keep });
		if (ok) newWord = '';
	}

	function removeWord(word: string): void {
		void commit({
			...$state.snapshot(vocabulary),
			keep: vocabulary.keep.filter((w) => w !== word)
		});
	}

	function onWordKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter') {
			event.preventDefault();
			void addWord();
		} else if (event.key === 'Backspace' && newWord === '' && vocabulary.keep.length > 0) {
			const last = vocabulary.keep[vocabulary.keep.length - 1];
			if (last) removeWord(last);
		}
	}

	function onWordPaste(event: ClipboardEvent): void {
		const text = event.clipboardData?.getData('text') ?? '';
		if (!/\r?\n/.test(text.trim())) return;
		event.preventDefault();
		newWord = text;
		void addWord();
	}

	function startEdit(index: number | 'new'): void {
		const pair = index === 'new' ? undefined : vocabulary.replacements[index];
		fromValue = pair?.from ?? '';
		toValue = pair?.to ?? '';
		editing = index;
		queueMicrotask(() => fromInput?.focus());
	}

	function cancelEdit(): void {
		editing = null;
	}

	async function saveEdit(): Promise<void> {
		if (editing === null) return;
		if (!fromValue.trim()) {
			cancelEdit();
			return;
		}
		const replacements = upsertReplacement(
			vocabulary.replacements,
			{ from: fromValue, to: toValue },
			editing === 'new' ? null : editing
		);
		const ok = await commit({ ...$state.snapshot(vocabulary), replacements });
		if (ok) editing = null;
	}

	function removeReplacement(index: number): void {
		editing = null;
		void commit({
			...$state.snapshot(vocabulary),
			replacements: vocabulary.replacements.filter((_, i) => i !== index)
		});
	}

	function onPairKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter') {
			event.preventDefault();
			void saveEdit();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			event.stopPropagation();
			cancelEdit();
		}
	}
</script>

{#snippet pairEditor()}
	<div class="list-row gap-3 py-2.5">
		<input
			bind:this={fromInput}
			bind:value={fromValue}
			onkeydown={onPairKeydown}
			placeholder={i18n.t.vocabulary.replaceFromPlaceholder}
			aria-label={i18n.t.vocabulary.replaceFrom}
			autocomplete="off"
			spellcheck="false"
			class="h-8 min-w-0 flex-1 rounded-[8px] border border-input bg-card px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
		/>
		<ArrowRightIcon class="size-4 shrink-0 text-muted-foreground/60" />
		<input
			bind:value={toValue}
			onkeydown={onPairKeydown}
			placeholder={i18n.t.vocabulary.replaceToPlaceholder}
			aria-label={i18n.t.vocabulary.replaceTo}
			autocomplete="off"
			spellcheck="false"
			class="h-8 min-w-0 flex-1 rounded-[8px] border border-input bg-card px-2.5 text-sm outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/30"
		/>
		<Button variant="ghost" size="sm" onclick={cancelEdit}>{i18n.t.common.cancel}</Button>
		<Button size="sm" onclick={() => void saveEdit()} disabled={!fromValue.trim()}
			>{i18n.t.vocabulary.replaceSave}</Button
		>
	</div>
{/snippet}

<div class="flex h-full min-h-0 flex-col">
	<PageHeader title={i18n.t.vocabulary.title} />

	<div class="page-body">
		{#if saveError}
			<Alert.Root variant="destructive">
				<Alert.Title>{i18n.t.vocabulary.saveErrorTitle}</Alert.Title>
				<Alert.Description>{saveError}</Alert.Description>
			</Alert.Root>
		{/if}

		<section>
			<div class="group-title">
				<span>{i18n.t.vocabulary.keepTitle}</span>
				<span class="text-xs text-muted-foreground/80">{i18n.t.vocabulary.keepHint}</span>
			</div>
			<div class="list-card">
				<div class="flex flex-wrap items-center gap-1.5 p-4">
					{#each vocabulary.keep as word (word)}
						<span
							class="inline-flex h-7 max-w-full items-center gap-0.5 rounded-full border bg-card pr-1 pl-3 text-[13px]"
						>
							<span class="truncate select-text">{word}</span>
							<button
								type="button"
								class="grid size-5 shrink-0 place-items-center rounded-full text-muted-foreground hover:bg-accent hover:text-foreground"
								onclick={() => removeWord(word)}
								aria-label={i18n.t.vocabulary.keepRemove(word)}
								title={i18n.t.vocabulary.keepRemove(word)}
							>
								<XIcon class="size-3" />
							</button>
						</span>
					{/each}
					<div
						class="inline-flex h-7 min-w-44 flex-1 items-center gap-1 rounded-full border border-dashed pr-1 pl-3 focus-within:border-ring"
					>
						<input
							bind:value={newWord}
							onkeydown={onWordKeydown}
							onpaste={onWordPaste}
							placeholder={i18n.t.vocabulary.keepPlaceholder}
							aria-label={i18n.t.vocabulary.keepPlaceholder}
							autocomplete="off"
							spellcheck="false"
							class="h-full min-w-0 flex-1 bg-transparent text-[13px] outline-none placeholder:text-muted-foreground"
						/>
						{#if newWord.trim()}
							<button
								type="button"
								class="rounded-full px-2 py-0.5 text-xs text-muted-foreground hover:bg-accent hover:text-foreground"
								onclick={() => void addWord()}>{i18n.t.vocabulary.keepAdd}</button
							>
						{/if}
					</div>
				</div>
				{#if vocabulary.keep.length === 0}
					<p class="-mt-1 px-5 pb-4 text-[13px] text-muted-foreground">
						{i18n.t.vocabulary.keepEmpty}
					</p>
				{/if}
			</div>
		</section>

		<section>
			<div class="group-title">
				<span>{i18n.t.vocabulary.replaceTitle}</span>
				<span class="text-xs text-muted-foreground/80">{i18n.t.vocabulary.replaceHint}</span>
			</div>
			<div class="list-card">
				{#each vocabulary.replacements as pair, i (i)}
					{#if editing === i}
						{@render pairEditor()}
					{:else}
						<div class="group list-row gap-3 py-0 pr-3">
							<button
								type="button"
								class="flex min-w-0 flex-1 items-center gap-3 py-3 text-left"
								onclick={() => startEdit(i)}
								aria-label={i18n.t.vocabulary.replaceEditAria(pair.from)}
							>
								<span class="min-w-0 flex-1 truncate">{pair.from}</span>
								<ArrowRightIcon class="size-4 shrink-0 text-muted-foreground/60" />
								<span
									class={['min-w-0 flex-1 truncate', !pair.to && 'text-muted-foreground italic']}
									>{pair.to || i18n.t.vocabulary.replaceDeleted}</span
								>
							</button>
							<button
								type="button"
								class="grid size-7 shrink-0 place-items-center rounded-md text-muted-foreground opacity-0 group-hover:opacity-100 hover:bg-accent hover:text-foreground focus-visible:opacity-100"
								onclick={() => removeReplacement(i)}
								aria-label={i18n.t.vocabulary.replaceRemove(pair.from)}
								title={i18n.t.vocabulary.replaceRemove(pair.from)}
							>
								<TrashIcon class="size-3.5" />
							</button>
						</div>
					{/if}
				{/each}
				{#if editing === 'new'}
					{@render pairEditor()}
				{:else}
					<button
						type="button"
						class="list-row py-3 text-muted-foreground"
						onclick={() => startEdit('new')}
					>
						<PlusIcon class="size-4 shrink-0" />
						<span>{i18n.t.vocabulary.replaceAdd}</span>
					</button>
				{/if}
			</div>
		</section>

		<p class="px-1 text-[13px] leading-relaxed text-muted-foreground">{i18n.t.vocabulary.note}</p>
	</div>
</div>
