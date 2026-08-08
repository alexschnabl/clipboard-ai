<!-- lib/components/CommandList.svelte — table of configured commands: add / edit / delete -->
<script lang="ts">
	import * as Table from '$lib/components/shadcn/table/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Badge } from '$lib/components/shadcn/badge/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import RiAddLine from 'remixicon-svelte/icons/add-line';
	import RiPencilLine from 'remixicon-svelte/icons/pencil-line';
	import RiDeleteBinLine from 'remixicon-svelte/icons/delete-bin-line';
	import CommandForm from './CommandForm.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import type { CommandConfig, Problem } from '$lib/types';

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
		await configState.deleteCommand(target.id);
	}

	function cancelDelete(): void {
		confirmDeleteOpen = false;
		pendingDelete = null;
	}

	function backendName(backendId: string): string {
		return configState.config.backends.find((b) => b.id === backendId)?.name ?? 'Unknown backend';
	}

	/** Each `Problem` carries the id of the command it belongs to (see
	 *  `Problem` in src-tauri/src/lib.rs) rather than a string prefix to
	 *  match against, so this is a direct equality filter -- no fragile
	 *  reconciliation with how Rust happens to have formatted `message`. */
	function problemsFor(command: CommandConfig): Problem[] {
		return configState.problems.filter((p) => p.commandId === command.id);
	}

	// Dense sticky header strip, matching mockup-a-compact.html's `.col-headers`
	// (and copied verbatim from HistoryPanel's own `headClass` rather than a
	// shared import -- this project's existing idiom for this small a style
	// string, see that component). `sticky top-0` sticks to the nearest
	// scrolling ancestor, which is this component's own `content-scroll` div
	// below, not the page -- each section scrolls independently now.
	const headClass =
		'sticky top-0 z-10 h-7 bg-muted px-2 text-[10px] font-semibold tracking-wide text-muted-foreground uppercase';
</script>

<!-- `content-toolbar` (pinned, border-b) + `content-scroll` (scrolls, padded)
     split, matching mockup-a-compact.html's `.content` structure -- see
     +page.svelte's comment on why this replaced one page-level scroll region.
     `px-4`/`py-2.5` here is the 16px/10px toolbar padding the mockup uses for
     every section's toolbar; the `p-4` on content-scroll below is the same
     16px gutter applied to the body, which is what was missing before (the
     reported bug: content sitting flush against the sidebar). -->
<div class="flex h-full min-h-0 flex-col">
	<div class="flex shrink-0 items-center gap-2 border-b px-4 py-2.5">
		<h2 class="text-[12.5px] font-bold">Commands</h2>
		<span class="text-[11px] text-muted-foreground"
			>{configState.config.commands.length} configured</span
		>
		<div class="flex-1"></div>
		<Button size="sm" onclick={openAdd}>
			<RiAddLine class="size-4" />
			Add command
		</Button>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-4">
		<Table.Root>
			<Table.Header>
				<Table.Row class="hover:bg-transparent">
					<Table.Head class={headClass}>Name</Table.Head>
					<Table.Head class={headClass}>Hotkey</Table.Head>
					<Table.Head class={headClass}>Backend</Table.Head>
					<Table.Head class="{headClass} text-right">Actions</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each configState.config.commands as command (command.id)}
					{@const problems = problemsFor(command)}
					<Table.Row>
						<Table.Cell class="max-w-48 py-1.5 font-medium">
							<span class="block truncate" title={command.name}>{command.name}</span>
							{#if problems.length > 0}
								<p class="mt-1 text-xs font-normal whitespace-normal text-destructive">
									{problems.map((p) => p.message).join(' ')}
								</p>
							{/if}
						</Table.Cell>
						<Table.Cell class="py-1.5">
							{#if command.hotkey}
								<Badge variant="outline" class="font-mono">{command.hotkey}</Badge>
							{:else}
								<span class="text-muted-foreground">—</span>
							{/if}
						</Table.Cell>
						<Table.Cell class="max-w-48 py-1.5">
							<span class="block truncate" title={backendName(command.backendId)}
								>{backendName(command.backendId)}</span
							>
						</Table.Cell>
						<Table.Cell class="py-1.5 text-right">
							<div class="flex justify-end gap-2">
								<Button
									variant="outline"
									size="icon-sm"
									onclick={() => openEdit(command)}
									aria-label={`Edit ${command.name}`}
								>
									<RiPencilLine class="size-4" />
								</Button>
								<Button
									variant="outline"
									size="icon-sm"
									onclick={() => handleDelete(command)}
									aria-label={`Delete ${command.name}`}
								>
									<RiDeleteBinLine class="size-4" />
								</Button>
							</div>
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={4} class="py-8 text-center text-muted-foreground">
							No commands configured.
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
</div>

<CommandForm bind:open={formOpen} command={editingCommand} />

<Dialog.Root bind:open={confirmDeleteOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Delete “{pendingDelete?.name}”?</Dialog.Title>
			<Dialog.Description>
				This removes the command and its hotkey permanently. This can’t be undone.
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={cancelDelete}>Cancel</Button>
			<Button variant="destructive" onclick={() => void confirmDelete()}>Delete</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
