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
	import { TABLE_HEAD_CLASS } from '$lib/table';
	import { i18n } from '$lib/i18n/i18n.svelte';
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

<div class="flex h-full min-h-0 flex-col">
	<div class="flex shrink-0 items-center gap-2 border-b px-4 py-2.5">
		<h2 class="text-[12.5px] font-bold">{i18n.t.backends.title}</h2>
		<span class="text-[11px] text-muted-foreground"
			>{i18n.t.backends.count(configState.config.backends.length)}</span
		>
		<div class="flex-1"></div>
		<Button size="sm" onclick={openAdd}>
			<RiAddLine class="size-4" />
			{i18n.t.backends.addBackend}
		</Button>
	</div>

	<div class="flex min-h-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
		{#if blockedDelete}
			<Alert.Root variant="destructive">
				<Alert.Title>{i18n.t.backends.blockedDeleteTitle(blockedDelete.backendName)}</Alert.Title>
				<Alert.Description>
					{i18n.t.backends.blockedDeleteDescription(blockedDelete.usedBy.join(', '))}
				</Alert.Description>
			</Alert.Root>
		{/if}

		<Table.Root>
			<Table.Header>
				<Table.Row class="hover:bg-transparent">
					<Table.Head class={TABLE_HEAD_CLASS}>{i18n.t.backends.columnName}</Table.Head>
					<Table.Head class={TABLE_HEAD_CLASS}>{i18n.t.backends.columnKind}</Table.Head>
					<Table.Head class={TABLE_HEAD_CLASS}>{i18n.t.backends.columnModel}</Table.Head>
					<Table.Head class="{TABLE_HEAD_CLASS} text-right">{i18n.t.backends.columnActions}</Table.Head>
				</Table.Row>
			</Table.Header>
			<Table.Body>
				{#each configState.config.backends as backend (backend.id)}
					<Table.Row>
						<Table.Cell class="max-w-48 py-1.5 font-medium">
							<span class="block truncate" title={backend.name}>{backend.name}</span>
						</Table.Cell>
						<Table.Cell class="py-1.5">{KIND_LABELS[backend.kind]}</Table.Cell>
						<Table.Cell class="max-w-48 py-1.5">
							<span class="block truncate" title={backend.model ?? undefined}
								>{backend.model ?? '—'}</span
							>
						</Table.Cell>
						<Table.Cell class="py-1.5 text-right">
							<div class="flex justify-end gap-2">
								<Button
									variant="outline"
									size="icon-sm"
									onclick={() => openEdit(backend)}
									aria-label={i18n.t.backends.editAria(backend.name)}
								>
									<RiPencilLine class="size-4" />
								</Button>
								<Button
									variant="outline"
									size="icon-sm"
									onclick={() => handleDelete(backend)}
									aria-label={i18n.t.backends.deleteAria(backend.name)}
								>
									<RiDeleteBinLine class="size-4" />
								</Button>
							</div>
						</Table.Cell>
					</Table.Row>
				{:else}
					<Table.Row>
						<Table.Cell colspan={4} class="py-8 text-center text-muted-foreground">
							{i18n.t.backends.empty}
						</Table.Cell>
					</Table.Row>
				{/each}
			</Table.Body>
		</Table.Root>
	</div>
</div>

<BackendForm bind:open={formOpen} backend={editingBackend} />

<Dialog.Root bind:open={confirmDeleteOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{i18n.t.backends.deleteTitle(pendingDelete?.name ?? '')}</Dialog.Title>
			<Dialog.Description>
				{i18n.t.backends.deleteDescription}
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
