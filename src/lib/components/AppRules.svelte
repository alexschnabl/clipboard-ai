<script lang="ts">
	import PlusIcon from '@lucide/svelte/icons/plus';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import AppIcon from './AppIcon.svelte';
	import AppRuleDialog from './AppRuleDialog.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { runningApps } from '$lib/state/apps.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { displayExe } from '$lib/editor';
	import type { AppRule, Formatting } from '$lib/types';

	let {
		rules = $bindable([]),
		commandBackendId,
		unsaved = false
	}: {
		rules: AppRule[];
		commandBackendId: string;
		unsaved?: boolean;
	} = $props();

	let dialogOpen = $state(false);
	let editIndex = $state<number | null>(null);

	$effect(() => {
		if (rules.length > 0) runningApps.ensure();
	});

	const formattingLabels = $derived<Record<Formatting, string>>({
		auto: i18n.t.commands.form.formattingOptionAuto,
		rich: i18n.t.commands.form.formattingOptionRich,
		plain: i18n.t.commands.form.formattingOptionPlain
	});

	function summary(rule: AppRule): string {
		const parts: string[] = [];
		if (rule.promptAddition.trim()) parts.push(i18n.t.commands.rules.adds(rule.promptAddition.trim()));
		if (rule.backendId) {
			const name = configState.config.backends.find((b) => b.id === rule.backendId)?.name;
			if (name) parts.push(i18n.t.commands.rules.uses(name));
		}
		if (rule.formatting) parts.push(formattingLabels[rule.formatting]);
		return parts.length > 0 ? parts.join(' - ') : i18n.t.commands.rules.nothing;
	}

	function openAdd(): void {
		editIndex = null;
		dialogOpen = true;
	}

	function openEdit(index: number): void {
		editIndex = index;
		dialogOpen = true;
	}

	function save(rule: AppRule, index: number | null): void {
		if (index === null) rules = [...rules, rule];
		else rules = rules.map((r, i) => (i === index ? rule : r));
	}

	function remove(index: number): void {
		rules = rules.filter((_, i) => i !== index);
	}
</script>

<section class="shrink-0">
	<div class="group-title">
		<span>{i18n.t.commands.rules.title}</span>
		<span class="text-xs text-muted-foreground/80">{i18n.t.commands.rules.hint}</span>
	</div>
	<div class="list-card">
		{#each rules as rule, i (i)}
			{@const label = rule.name.trim() || displayExe(rule.exe)}
			<button
				type="button"
				class="list-row py-3"
				onclick={() => openEdit(i)}
				aria-label={i18n.t.commands.rules.editAria(label)}
			>
				<AppIcon exe={rule.exe} class="size-6" />
				<div class="min-w-0 flex-1">
					<div class="flex min-w-0 items-baseline gap-2">
						<span class="truncate">{label}</span>
						{#if label.toLowerCase() !== displayExe(rule.exe).toLowerCase()}
							<span class="shrink-0 text-xs text-muted-foreground/80">{displayExe(rule.exe)}</span>
						{/if}
					</div>
					<div class="mt-0.5 truncate text-[13px] text-muted-foreground">{summary(rule)}</div>
				</div>
				<ChevronRightIcon class="size-4 shrink-0 text-muted-foreground/60" />
			</button>
		{/each}
		<button type="button" class="list-row py-3 text-muted-foreground" onclick={openAdd}>
			<span class="grid size-6 shrink-0 place-items-center"><PlusIcon class="size-4" /></span>
			<span>{i18n.t.commands.rules.add}</span>
		</button>
	</div>
	{#if unsaved}
		<p class="px-1 pt-2 text-xs text-muted-foreground">{i18n.t.commands.rules.savedWithCommand}</p>
	{/if}
</section>

<AppRuleDialog
	bind:open={dialogOpen}
	{rules}
	{editIndex}
	{commandBackendId}
	onSave={save}
	onDelete={remove}
/>
