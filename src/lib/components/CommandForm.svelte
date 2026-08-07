<!-- lib/components/CommandForm.svelte — add/edit dialog for a single command -->
<script lang="ts">
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import { Textarea } from '$lib/components/shadcn/textarea/index.js';
	import HotkeyRecorder from './HotkeyRecorder.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import type { CommandConfig } from '$lib/types';

	let {
		open = $bindable(false),
		command = null
	}: {
		open: boolean;
		/** The command being edited, or null when adding a new one. */
		command?: CommandConfig | null;
	} = $props();

	let name = $state('');
	let backendId = $state('');
	let hotkey = $state('');
	let systemPrompt = $state('');
	let saving = $state(false);
	/** Set from `configState.error` when `save()` (via `commit`) rejects this
	 *  dialog's edit -- shown inline, dialog left open, so the user can fix
	 *  the field and retry (see fix3 report). Reset on open, not just on a
	 *  fresh attempt, so a stale error from a *previous* dialog session never
	 *  bleeds into one that hasn't tried to save yet. */
	let submitError = $state<string | null>(null);

	const isEdit = $derived(command !== null);
	const canSave = $derived(name.trim().length > 0 && backendId.length > 0);
	const backendName = $derived(
		configState.config.backends.find((b) => b.id === backendId)?.name ?? 'Select a backend'
	);

	let wasOpen = false;
	// Seed the form whenever the dialog opens; when it closes -- Save,
	// Cancel, Escape or a backdrop click all flow through here since `open`
	// is the one signal common to all of them -- clear any stale
	// `configState.error` so the page-level banner in +page.svelte doesn't
	// keep showing a failed-save error for an edit the user just abandoned
	// (a successful save has already nulled it out, so this is a no-op then).
	$effect(() => {
		if (open) {
			wasOpen = true;
			name = command?.name ?? '';
			backendId = command?.backendId ?? configState.config.backends[0]?.id ?? '';
			hotkey = command?.hotkey ?? '';
			systemPrompt = command?.systemPrompt ?? '';
			submitError = null;
		} else if (wasOpen) {
			wasOpen = false;
			configState.error = null;
		}
	});

	function handleBackendChange(value: string | undefined): void {
		if (typeof value !== 'string') return;
		backendId = value;
	}

	function handleHotkeyChange(next: string): void {
		hotkey = next;
	}

	async function handleSave(): Promise<void> {
		if (!canSave || saving) return;
		const next: CommandConfig = {
			id: command?.id ?? crypto.randomUUID(),
			name: name.trim(),
			backendId,
			hotkey,
			systemPrompt: systemPrompt.trim()
		};
		saving = true;
		submitError = null;
		const ok = command ? await configState.updateCommand(next) : await configState.addCommand(next);
		saving = false;
		if (ok) {
			open = false;
		} else {
			submitError = configState.error?.message ?? 'Save failed.';
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{isEdit ? 'Edit command' : 'Add command'}</Dialog.Title>
			<Dialog.Description>
				{isEdit
					? 'Update this command’s backend, hotkey and prompt.'
					: 'Configure a new command.'}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-4 py-2">
			<div class="flex flex-col gap-1.5">
				<Label for="command-name">Name</Label>
				<Input id="command-name" bind:value={name} placeholder="Fix grammar" autocomplete="off" />
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-backend">Backend</Label>
				<Select.Root type="single" value={backendId} onValueChange={handleBackendChange}>
					<Select.Trigger id="command-backend" class="w-full">
						{backendName}
					</Select.Trigger>
					<Select.Content>
						{#each configState.config.backends as backend (backend.id)}
							<Select.Item value={backend.id} label={backend.name} />
						{/each}
					</Select.Content>
				</Select.Root>
				{#if configState.config.backends.length === 0}
					<p class="text-xs text-muted-foreground">
						No backends yet — add one on the Backends tab first.
					</p>
				{/if}
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-hotkey">Hotkey</Label>
				<HotkeyRecorder id="command-hotkey" value={hotkey} onchange={handleHotkeyChange} />
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-prompt">System prompt</Label>
				<Textarea
					id="command-prompt"
					bind:value={systemPrompt}
					placeholder="Fix grammar and spelling. Keep the meaning."
					rows={4}
				/>
			</div>
		</div>

		{#if submitError}
			<Alert.Root variant="destructive">
				<Alert.Title>Couldn’t save command</Alert.Title>
				<Alert.Description>{submitError}</Alert.Description>
			</Alert.Root>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)} disabled={saving}>Cancel</Button>
			<Button onclick={() => void handleSave()} disabled={!canSave || saving}>
				{saving ? 'Saving…' : isEdit ? 'Save changes' : 'Add command'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
