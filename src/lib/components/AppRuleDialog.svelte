<script lang="ts">
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import { Textarea } from '$lib/components/shadcn/textarea/index.js';
	import SearchIcon from '@lucide/svelte/icons/search';
	import RefreshIcon from '@lucide/svelte/icons/refresh-cw';
	import TrashIcon from '@lucide/svelte/icons/trash';
	import AppIcon from './AppIcon.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { runningApps } from '$lib/state/apps.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { displayExe, hasRuleFor, normalizeExe, ruleIsEmpty } from '$lib/editor';
	import type { AppRule, Formatting, RunningApp } from '$lib/types';

	let {
		open = $bindable(false),
		rules,
		editIndex,
		commandBackendId,
		onSave,
		onDelete
	}: {
		open: boolean;
		rules: AppRule[];
		editIndex: number | null;
		commandBackendId: string;
		onSave: (rule: AppRule, index: number | null) => void;
		onDelete: (index: number) => void;
	} = $props();

	const SAME = '__same__';
	const FORMATTING_VALUES: readonly Formatting[] = ['auto', 'rich', 'plain'];

	let exe = $state('');
	let name = $state('');
	let promptAddition = $state('');
	let backendChoice = $state(SAME);
	let formattingChoice = $state(SAME);
	let mode = $state<'pick' | 'manual' | 'chosen'>('pick');
	let query = $state('');
	let problem = $state<string | null>(null);
	let searchRef = $state<HTMLInputElement | null>(null);
	let promptRef = $state<HTMLTextAreaElement | null>(null);

	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			const rule = editIndex === null ? undefined : rules[editIndex];
			exe = rule?.exe ?? '';
			name = rule?.name ?? '';
			promptAddition = rule?.promptAddition ?? '';
			backendChoice = rule?.backendId ?? SAME;
			formattingChoice = rule?.formatting ?? SAME;
			mode = rule ? 'chosen' : 'pick';
			query = '';
			problem = null;
			void runningApps.refresh();
		}
		wasOpen = open;
	});

	$effect(() => {
		void [exe, promptAddition, backendChoice, formattingChoice];
		problem = null;
	});

	const isEdit = $derived(editIndex !== null);
	const shownName = $derived(name.trim() || displayExe(exe));

	const filtered = $derived.by(() => {
		const q = query.trim().toLowerCase();
		const list = runningApps.list;
		if (!q) return list;
		return list.filter((app) =>
			[app.name, app.exe, app.title].some((v) => v.toLowerCase().includes(q))
		);
	});

	const effectiveBackend = $derived(
		configState.config.backends.find(
			(b) => b.id === (backendChoice === SAME ? commandBackendId : backendChoice)
		)
	);
	const formattingApplies = $derived(effectiveBackend?.kind !== 'http');

	const formattingLabels = $derived<Record<Formatting, string>>({
		auto: i18n.t.commands.form.formattingOptionAuto,
		rich: i18n.t.commands.form.formattingOptionRich,
		plain: i18n.t.commands.form.formattingOptionPlain
	});

	function pick(app: RunningApp): void {
		exe = app.exe;
		name = app.name;
		mode = 'chosen';
		problem = null;
		queueMicrotask(() => promptRef?.focus());
	}

	function buildRule(): AppRule {
		const formatting =
			formattingApplies && formattingChoice !== SAME ? (formattingChoice as Formatting) : null;
		return {
			exe: displayExe(exe),
			name: name.trim() || displayExe(exe),
			promptAddition: promptAddition.trim(),
			backendId: backendChoice === SAME ? null : backendChoice,
			formatting
		};
	}

	function save(): void {
		if (!normalizeExe(exe)) {
			problem = i18n.t.commands.rules.pickApp;
			return;
		}
		if (hasRuleFor(rules, exe, editIndex ?? undefined)) {
			problem = i18n.t.commands.rules.duplicate;
			return;
		}
		const rule = buildRule();
		if (ruleIsEmpty(rule)) {
			problem = i18n.t.commands.rules.empty;
			return;
		}
		onSave(rule, editIndex);
		open = false;
	}

	function remove(): void {
		if (editIndex === null) return;
		onDelete(editIndex);
		open = false;
	}

	function onSearchKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && filtered.length === 1 && filtered[0]) {
			event.preventDefault();
			pick(filtered[0]);
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content
		class="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-lg"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			(mode === 'pick' ? searchRef : promptRef)?.focus();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>
				{isEdit ? i18n.t.commands.rules.editTitle(shownName) : i18n.t.commands.rules.addTitle}
			</Dialog.Title>
			<Dialog.Description>{i18n.t.commands.rules.description}</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-1.5">
			<div class="flex items-center justify-between gap-2">
				<Label for={mode === 'manual' ? 'rule-exe' : 'rule-app-search'}
					>{i18n.t.commands.rules.appLabel}</Label
				>
				{#if mode === 'pick'}
					<button
						type="button"
						class="text-xs text-muted-foreground hover:text-foreground"
						onclick={() => (mode = 'manual')}>{i18n.t.commands.rules.manual}</button
					>
				{:else if mode === 'manual'}
					<button
						type="button"
						class="text-xs text-muted-foreground hover:text-foreground"
						onclick={() => (mode = 'pick')}>{i18n.t.commands.rules.pickFromList}</button
					>
				{/if}
			</div>

			{#if mode === 'chosen'}
				<div class="flex items-center gap-3 rounded-[10px] border px-3 py-2">
					<AppIcon exe={exe} class="size-6" />
					<div class="min-w-0 flex-1">
						<div class="truncate">{shownName}</div>
						<div class="truncate text-xs text-muted-foreground">{displayExe(exe)}</div>
					</div>
					<Button variant="ghost" size="sm" onclick={() => (mode = 'pick')}
						>{i18n.t.commands.rules.change}</Button
					>
				</div>
			{:else if mode === 'pick'}
				<div class="overflow-hidden rounded-[10px] border">
					<div class="flex items-center gap-2 border-b px-3">
						<SearchIcon class="size-4 shrink-0 text-muted-foreground" />
						<input
							id="rule-app-search"
							bind:this={searchRef}
							bind:value={query}
							onkeydown={onSearchKeydown}
							placeholder={i18n.t.commands.rules.search}
							autocomplete="off"
							class="h-9 min-w-0 flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
						/>
						<button
							type="button"
							class="grid size-7 place-items-center rounded-md text-muted-foreground hover:bg-accent hover:text-foreground disabled:opacity-50"
							onclick={() => void runningApps.refresh()}
							disabled={runningApps.loading}
							aria-label={i18n.t.commands.rules.refresh}
							title={i18n.t.commands.rules.refresh}
						>
							<RefreshIcon class={['size-3.5', runningApps.loading && 'animate-spin']} />
						</button>
					</div>
					<div class="h-48 overflow-y-auto p-1">
						{#if runningApps.loading && runningApps.list.length === 0}
							<p class="px-3 py-6 text-center text-xs text-muted-foreground">
								{i18n.t.commands.rules.loading}
							</p>
						{:else if runningApps.list.length === 0}
							<p class="px-3 py-6 text-center text-xs text-muted-foreground">
								{i18n.t.commands.rules.none}
							</p>
						{:else}
							{#each filtered as app (app.exe)}
								{@const taken = hasRuleFor(rules, app.exe, editIndex ?? undefined)}
								<button
									type="button"
									class="flex w-full items-center gap-3 rounded-[8px] px-2.5 py-1.5 text-left hover:bg-accent disabled:opacity-45 disabled:hover:bg-transparent"
									onclick={() => pick(app)}
									disabled={taken}
									title={app.title}
								>
									<AppIcon exe={app.exe} class="size-5" />
									<span class="min-w-0 flex-1 truncate">{app.name}</span>
									<span class="max-w-40 shrink-0 truncate text-xs text-muted-foreground"
										>{app.exe}</span
									>
								</button>
							{:else}
								<p class="px-3 py-6 text-center text-xs text-muted-foreground">
									{i18n.t.commands.rules.noMatch}
								</p>
							{/each}
						{/if}
					</div>
				</div>
			{:else}
				<div class="grid grid-cols-2 gap-x-3 gap-y-1.5 rounded-[10px] border p-3">
					<div class="flex min-w-0 flex-col gap-1">
						<label for="rule-exe" class="text-xs text-muted-foreground"
							>{i18n.t.commands.rules.exeLabel}</label
						>
						<Input
							id="rule-exe"
							bind:value={exe}
							placeholder={i18n.t.commands.rules.exePlaceholder}
							autocomplete="off"
							spellcheck="false"
						/>
					</div>
					<div class="flex min-w-0 flex-col gap-1">
						<label for="rule-name" class="text-xs text-muted-foreground"
							>{i18n.t.commands.rules.nameLabel}</label
						>
						<Input
							id="rule-name"
							bind:value={name}
							placeholder={i18n.t.commands.rules.namePlaceholder}
							autocomplete="off"
						/>
					</div>
					<p class="col-span-2 text-xs text-muted-foreground">{i18n.t.commands.rules.exeHint}</p>
				</div>
			{/if}
		</div>

		<div class="flex flex-col gap-1.5">
			<Label for="rule-prompt">{i18n.t.commands.rules.promptLabel}</Label>
			<Textarea
				id="rule-prompt"
				bind:ref={promptRef}
				bind:value={promptAddition}
				rows={2}
				placeholder={i18n.t.commands.rules.promptPlaceholder}
				class="max-h-32 overflow-y-auto"
			/>
			<p class="text-xs text-muted-foreground">{i18n.t.commands.rules.promptHint}</p>
		</div>

		<div class="grid grid-cols-2 gap-3">
			<div class="flex min-w-0 flex-col gap-1.5">
				<Label for="rule-backend">{i18n.t.commands.rules.backendLabel}</Label>
				<Select.Root type="single" bind:value={backendChoice}>
					<Select.Trigger id="rule-backend" class="w-full">
						<span class="truncate">
							{backendChoice === SAME
								? i18n.t.commands.rules.same
								: (configState.config.backends.find((b) => b.id === backendChoice)?.name ?? '')}
						</span>
					</Select.Trigger>
					<Select.Content>
						<Select.Item value={SAME} label={i18n.t.commands.rules.same} />
						{#each configState.config.backends as backend (backend.id)}
							<Select.Item value={backend.id} label={backend.name} />
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
			<div class="flex min-w-0 flex-col gap-1.5">
				<Label for="rule-formatting">{i18n.t.commands.rules.formattingLabel}</Label>
				<Select.Root type="single" bind:value={formattingChoice} disabled={!formattingApplies}>
					<Select.Trigger id="rule-formatting" class="w-full">
						<span class="truncate">
							{formattingChoice === SAME || !formattingApplies
								? i18n.t.commands.rules.same
								: formattingLabels[formattingChoice as Formatting]}
						</span>
					</Select.Trigger>
					<Select.Content>
						<Select.Item value={SAME} label={i18n.t.commands.rules.same} />
						{#each FORMATTING_VALUES as value (value)}
							<Select.Item {value} label={formattingLabels[value]} />
						{/each}
					</Select.Content>
				</Select.Root>
			</div>
		</div>

		{#if problem}
			<p class="text-[13px] text-destructive" role="alert">{problem}</p>
		{/if}

		<Dialog.Footer class="items-center">
			{#if isEdit}
				<Button
					variant="ghost"
					class="mr-auto text-destructive hover:text-destructive"
					onclick={remove}
				>
					<TrashIcon class="size-4" />
					{i18n.t.commands.rules.delete}
				</Button>
			{/if}
			<Button variant="outline" onclick={() => (open = false)}>{i18n.t.common.cancel}</Button>
			<Button onclick={save}>
				{isEdit ? i18n.t.commands.rules.submitSave : i18n.t.commands.rules.submitAdd}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
