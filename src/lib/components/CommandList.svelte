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

	function confirmDelete(): void {
		if (!pendingDelete) return;
		configState.deleteCommand(pendingDelete.id);
		confirmDeleteOpen = false;
		pendingDelete = null;
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
</script>

<div class="flex flex-col gap-4">
	<div class="flex items-center justify-between">
		<h2 class="text-sm font-medium">Commands</h2>
		<Button size="sm" onclick={openAdd}>
			<RiAddLine class="size-4" />
			Add command
		</Button>
	</div>

	<Table.Root>
		<Table.Header>
			<Table.Row>
				<Table.Head>Name</Table.Head>
				<Table.Head>Hotkey</Table.Head>
				<Table.Head>Backend</Table.Head>
				<Table.Head class="text-right">Actions</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each configState.config.commands as command (command.id)}
				{@const problems = problemsFor(command)}
				<Table.Row>
					<Table.Cell class="font-medium">
						{command.name}
						{#if problems.length > 0}
							<p class="mt-1 text-xs font-normal text-destructive">
								{problems.map((p) => p.message).join(' ')}
							</p>
						{/if}
					</Table.Cell>
					<Table.Cell>
						{#if command.hotkey}
							<Badge variant="outline" class="font-mono">{command.hotkey}</Badge>
						{:else}
							<span class="text-muted-foreground">—</span>
						{/if}
					</Table.Cell>
					<Table.Cell>{backendName(command.backendId)}</Table.Cell>
					<Table.Cell class="text-right">
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
					<Table.Cell colspan={4} class="text-center text-muted-foreground">
						No commands configured.
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
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
			<Button variant="destructive" onclick={confirmDelete}>Delete</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
