<script lang="ts">
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { fade } from 'svelte/transition';
	import CommandForm from './CommandForm.svelte';
	import GettingStarted from './GettingStarted.svelte';
	import Keycaps from './Keycaps.svelte';
	import PageHeader from './PageHeader.svelte';
	import StatusLine from './StatusLine.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { healthState } from '$lib/state/health.svelte.js';
	import { onboarding } from '$lib/state/onboarding.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { classifyCommandProblem, commandProblemSentence } from '$lib/problem-format';
	import type { CommandConfig, Problem } from '$lib/types';

	let { onOpenBackends = () => {} }: { onOpenBackends?: () => void } = $props();

	const showGettingStarted = $derived(
		onboarding.shouldShow({
			dismissed: configState.config.settings.onboardingDismissed === true,
			isDefaultSetup: configState.isDefaultSetup,
			allBackendsFailing: healthState.allCheckedAndFailing(
				configState.config.backends.map((b) => b.id)
			)
		})
	);

	$effect(() => {
		if (showGettingStarted) onboarding.sticky = true;
	});

	let formOpen = $state(false);
	let editingCommand = $state<CommandConfig | null>(null);
	let pendingDelete = $state<CommandConfig | null>(null);
	let confirmDeleteOpen = $state(false);

	function openAdd(): void {
		editingCommand = null;
		formOpen = true;
	}

	function openEdit(command: CommandConfig): void {
		editingCommand = command;
		formOpen = true;
	}

	function handleDelete(command: CommandConfig): void {
		pendingDelete = command;
		confirmDeleteOpen = true;
	}

	async function confirmDelete(): Promise<void> {
		if (!pendingDelete) return;
		const target = pendingDelete;
		confirmDeleteOpen = false;
		pendingDelete = null;
		formOpen = false;
		await configState.deleteCommand(target.id);
	}

	function cancelDelete(): void {
		confirmDeleteOpen = false;
		pendingDelete = null;
	}

	function backendName(backendId: string): string {
		return (
			configState.config.backends.find((b) => b.id === backendId)?.name ??
			i18n.t.commands.unknownBackend
		);
	}

	function sameHotkey(a: string, b: string): boolean {
		const norm = (h: string) => h.replace(/\s+/g, '').toLowerCase();
		return norm(a) === norm(b);
	}

	function problemsFor(command: CommandConfig): Problem[] {
		const settings = configState.config.settings;
		const globalKeys: Record<string, string | undefined> = {
			palette: settings.paletteHotkey,
			'paste-last': settings.pasteLastEnabled ? settings.pasteLastHotkey : undefined
		};
		return configState.problems.filter((p) => {
			if (p.commandId === command.id) return true;
			const key = p.commandId ? globalKeys[p.commandId] : undefined;
			return !!key && !!command.hotkey && sameHotkey(key, command.hotkey);
		});
	}

	function problemText(command: CommandConfig, problems: Problem[]): string {
		return problems
			.map((p) =>
				commandProblemSentence(
					classifyCommandProblem(p.message, command.name),
					i18n.t.commands.problems
				)
			)
			.join(' ');
	}
</script>

{#if !formOpen}
	<div class="flex h-full min-h-0 flex-col" in:fade={{ duration: 120 }}>
		<PageHeader
			title={i18n.t.commands.title}
			subtitle={i18n.t.commands.count(configState.config.commands.length)}
		>
			{#snippet actions()}
				<Button size="sm" onclick={openAdd}>
					<PlusIcon class="size-4" />
					{i18n.t.commands.addCommand}
				</Button>
			{/snippet}
		</PageHeader>

		<div class="page-body">
			{#if showGettingStarted}
				<GettingStarted {onOpenBackends} />
			{/if}

			<section>
				<div class="group-title">
					<span>{i18n.t.commands.groupTitle}</span>
					<span class="text-xs text-muted-foreground/80">{i18n.t.commands.groupHint}</span>
				</div>
				<div class="list-card">
					{#each configState.config.commands as command (command.id)}
						{@const problems = problemsFor(command)}
						<button
							type="button"
							class="list-row"
							onclick={() => openEdit(command)}
							aria-label={i18n.t.commands.editAria(command.name)}
						>
							<div class="min-w-0 flex-1">
								<div class="truncate" title={command.name}>{command.name}</div>
								{#if problems.length > 0}
									<StatusLine
										tone="error"
										text={problemText(command, problems)}
										title={problems.map((p) => p.message).join('\n')}
										class="mt-1 text-[13px] text-destructive"
									/>
								{:else if command.systemPrompt}
									<div class="mt-1 truncate text-[13px] text-muted-foreground">
										{command.systemPrompt}
									</div>
								{/if}
							</div>
							<span
								class="max-w-40 shrink-0 truncate rounded-[5px] bg-muted px-2 py-0.5 text-[11.5px] text-muted-foreground"
								title={backendName(command.backendId)}>{backendName(command.backendId)}</span
							>
							{#if command.hotkey}
								<Keycaps hotkey={command.hotkey} />
							{/if}
							<ChevronRightIcon class="size-4 shrink-0 text-muted-foreground/60" />
						</button>
					{:else}
						<div class="px-5 py-8 text-center text-sm text-muted-foreground">
							{i18n.t.commands.empty}
						</div>
					{/each}
				</div>
			</section>
		</div>
	</div>
{/if}

<CommandForm
	bind:open={formOpen}
	command={editingCommand}
	onDelete={editingCommand ? () => editingCommand && handleDelete(editingCommand) : undefined}
/>

<Dialog.Root bind:open={confirmDeleteOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{i18n.t.commands.deleteTitle(pendingDelete?.name ?? '')}</Dialog.Title>
			<Dialog.Description>
				{i18n.t.commands.deleteDescription}
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={cancelDelete}>{i18n.t.common.cancel}</Button>
			<Button variant="destructive" onclick={() => void confirmDelete()}
				>{i18n.t.common.delete}</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
