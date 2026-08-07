<!-- lib/components/BackendList.svelte — table of configured backends: add / edit / delete -->
<script lang="ts">
	import * as Table from '$lib/components/shadcn/table/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import RiAddLine from 'remixicon-svelte/icons/add-line';
	import RiPencilLine from 'remixicon-svelte/icons/pencil-line';
	import RiDeleteBinLine from 'remixicon-svelte/icons/delete-bin-line';
	import BackendForm from './BackendForm.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { KIND_LABELS, type BackendConfig } from '$lib/types';

	let formOpen = $state(false);
	let editingBackend = $state<BackendConfig | null>(null);
	let blockedDelete = $state<{ backendName: string; usedBy: string[] } | null>(null);
	let pendingDelete = $state<BackendConfig | null>(null);
	let confirmDeleteOpen = $state(false);

	function openAdd(): void {
		editingBackend = null;
		blockedDelete = null;
		formOpen = true;
	}

	function openEdit(backend: BackendConfig): void {
		editingBackend = backend;
		blockedDelete = null;
		formOpen = true;
	}

	function handleDelete(backend: BackendConfig): void {
		pendingDelete = backend;
		confirmDeleteOpen = true;
	}

	/** The used-by refusal is unchanged: still attempted only after the user
	 *  confirms, still shown as the same destructive Alert naming the
	 *  blocking commands -- `deleteBackend` checks that locally and refuses
	 *  before any write, so it's the only way this call resolves with a
	 *  non-empty `usedBy`. The confirmation dialog itself closes immediately
	 *  on confirm, same as before; the write that follows now persists to
	 *  disk rather than only flipping a dirty flag. */
	async function confirmDelete(): Promise<void> {
		if (!pendingDelete) return;
		const target = pendingDelete;
		confirmDeleteOpen = false;
		pendingDelete = null;
		const result = await configState.deleteBackend(target.id);
		blockedDelete = result.usedBy.length > 0 ? { backendName: target.name, usedBy: result.usedBy } : null;
	}

	function cancelDelete(): void {
		confirmDeleteOpen = false;
		pendingDelete = null;
	}
</script>

<div class="flex flex-col gap-4">
	{#if blockedDelete}
		<Alert.Root variant="destructive">
			<Alert.Title>Can’t delete “{blockedDelete.backendName}”</Alert.Title>
			<Alert.Description>
				Still used by: {blockedDelete.usedBy.join(', ')}. Point those commands at a different
				backend first, then delete it.
			</Alert.Description>
		</Alert.Root>
	{/if}

	<div class="flex items-center justify-between">
		<h2 class="text-sm font-medium">Backends</h2>
		<Button size="sm" onclick={openAdd}>
			<RiAddLine class="size-4" />
			Add backend
		</Button>
	</div>

	<Table.Root>
		<Table.Header>
			<Table.Row>
				<Table.Head>Name</Table.Head>
				<Table.Head>Kind</Table.Head>
				<Table.Head>Model</Table.Head>
				<Table.Head class="text-right">Actions</Table.Head>
			</Table.Row>
		</Table.Header>
		<Table.Body>
			{#each configState.config.backends as backend (backend.id)}
				<Table.Row>
					<Table.Cell class="max-w-48 font-medium">
						<span class="block truncate" title={backend.name}>{backend.name}</span>
					</Table.Cell>
					<Table.Cell>{KIND_LABELS[backend.kind]}</Table.Cell>
					<Table.Cell class="max-w-48">
						<span class="block truncate" title={backend.model ?? undefined}
							>{backend.model ?? '—'}</span
						>
					</Table.Cell>
					<Table.Cell class="text-right">
						<div class="flex justify-end gap-2">
							<Button
								variant="outline"
								size="icon-sm"
								onclick={() => openEdit(backend)}
								aria-label={`Edit ${backend.name}`}
							>
								<RiPencilLine class="size-4" />
							</Button>
							<Button
								variant="outline"
								size="icon-sm"
								onclick={() => handleDelete(backend)}
								aria-label={`Delete ${backend.name}`}
							>
								<RiDeleteBinLine class="size-4" />
							</Button>
						</div>
					</Table.Cell>
				</Table.Row>
			{:else}
				<Table.Row>
					<Table.Cell colspan={4} class="text-center text-muted-foreground">
						No backends configured.
					</Table.Cell>
				</Table.Row>
			{/each}
		</Table.Body>
	</Table.Root>
</div>

<BackendForm bind:open={formOpen} backend={editingBackend} />

<Dialog.Root bind:open={confirmDeleteOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Delete “{pendingDelete?.name}”?</Dialog.Title>
			<Dialog.Description>
				This removes the backend permanently. This can’t be undone.
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={cancelDelete}>Cancel</Button>
			<Button variant="destructive" onclick={() => void confirmDelete()}>Delete</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
