<!-- lib/components/CommandList.svelte — table of configured commands: add / edit / delete -->
<script lang="ts">
	import * as Table from '$lib/components/shadcn/table/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Badge } from '$lib/components/shadcn/badge/index.js';
	import RiAddLine from 'remixicon-svelte/icons/add-line';
	import RiPencilLine from 'remixicon-svelte/icons/pencil-line';
	import RiDeleteBinLine from 'remixicon-svelte/icons/delete-bin-line';
	import CommandForm from './CommandForm.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import type { CommandConfig } from '$lib/types';

	let formOpen = $state(false);
	let editingCommand = $state<CommandConfig | null>(null);

	function openAdd(): void {
		editingCommand = null;
		formOpen = true;
	}

	function openEdit(command: CommandConfig): void {
		editingCommand = command;
		formOpen = true;
	}

	function handleDelete(command: CommandConfig): void {
		configState.deleteCommand(command.id);
	}

	function backendName(backendId: string): string {
		return configState.config.backends.find((b) => b.id === backendId)?.name ?? 'Unknown backend';
	}

	/**
	 * `configState.problems` entries arrive prefixed with either the command
	 * name ("Fix grammar: unknown backend") or the parsed hotkey
	 * ("Ctrl+Alt+J: already in use (...)") — see `build_command_map` and
	 * `register_hotkeys` in src-tauri/src/lib.rs. Best-effort match on those
	 * prefixes so a command's own problem shows on its own row; the aggregate
	 * Alert in +page.svelte is still shown as the fallback.
	 */
	function problemsFor(command: CommandConfig): string[] {
		return configState.problems.filter(
			(p) => p.startsWith(`${command.name}: `) || p.startsWith(`${command.hotkey}: `)
		);
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
							<p class="mt-1 text-xs font-normal text-destructive">{problems.join(' ')}</p>
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
