<!-- lib/components/BackendList.svelte — table of configured backends: add / edit / delete -->
<script lang="ts">
	import * as Table from '$lib/components/shadcn/table/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import RiAddLine from 'remixicon-svelte/icons/add-line';
	import RiPencilLine from 'remixicon-svelte/icons/pencil-line';
	import RiDeleteBinLine from 'remixicon-svelte/icons/delete-bin-line';
	import BackendForm from './BackendForm.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import type { BackendConfig, BackendKind } from '$lib/types';

	const KIND_LABELS: Record<BackendKind, string> = {
		'openai-compatible': 'OpenAI-compatible',
		'anthropic-api': 'Anthropic API',
		'claude-cli': 'Claude CLI'
	};

	let formOpen = $state(false);
	let editingBackend = $state<BackendConfig | null>(null);
	let blockedDelete = $state<{ backendName: string; usedBy: string[] } | null>(null);

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
		const result = configState.deleteBackend(backend.id);
		blockedDelete = result.ok ? null : { backendName: backend.name, usedBy: result.usedBy };
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
					<Table.Cell class="font-medium">{backend.name}</Table.Cell>
					<Table.Cell>{KIND_LABELS[backend.kind]}</Table.Cell>
					<Table.Cell>{backend.model ?? '—'}</Table.Cell>
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
