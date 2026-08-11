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
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { CommandConfig } from '$lib/types';

	let {
		open = $bindable(false),
		command = null
	}: {
		open: boolean;
		command?: CommandConfig | null;
	} = $props();

	let name = $state('');
	let backendId = $state('');
	let hotkey = $state('');
	let systemPrompt = $state('');
	let saving = $state(false);
	let submitError = $state<string | null>(null);

	const isEdit = $derived(command !== null);
	const canSave = $derived(name.trim().length > 0 && backendId.length > 0);
	const backendName = $derived(
		configState.config.backends.find((b) => b.id === backendId)?.name ??
			i18n.t.commands.form.backendPlaceholder
	);
	const backendIsHttp = $derived(
		configState.config.backends.find((b) => b.id === backendId)?.kind === 'http'
	);

	let wasOpen = false;
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
			submitError = configState.error?.message ?? i18n.t.commands.form.saveFailedFallback;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{isEdit ? i18n.t.commands.form.editTitle : i18n.t.commands.form.addTitle}</Dialog.Title>
			<Dialog.Description>
				{isEdit ? i18n.t.commands.form.editDescription : i18n.t.commands.form.addDescription}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-3">
			<div class="flex flex-col gap-1.5">
				<Label for="command-name">{i18n.t.commands.form.nameLabel}</Label>
				<Input
					id="command-name"
					bind:value={name}
					placeholder={i18n.t.commands.form.namePlaceholder}
					autocomplete="off"
				/>
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-backend">{i18n.t.commands.form.backendLabel}</Label>
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
						{i18n.t.commands.form.noBackendsHint}
					</p>
				{/if}
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-hotkey">{i18n.t.commands.form.hotkeyLabel}</Label>
				<HotkeyRecorder id="command-hotkey" value={hotkey} onchange={handleHotkeyChange} />
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="command-prompt">{i18n.t.commands.form.promptLabel}</Label>
				<Textarea
					id="command-prompt"
					bind:value={systemPrompt}
					placeholder={i18n.t.commands.form.promptPlaceholder}
					rows={4}
					class="max-h-48 overflow-y-auto"
				/>
				<p class="text-xs text-muted-foreground">
					{backendIsHttp
						? i18n.t.commands.form.promptHttpHint
						: i18n.t.commands.form.promptSentinelHint}
				</p>
			</div>
		</div>

		{#if submitError}
			<Alert.Root variant="destructive">
				<Alert.Title>{i18n.t.commands.form.saveError}</Alert.Title>
				<Alert.Description>{submitError}</Alert.Description>
			</Alert.Root>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)} disabled={saving}
				>{i18n.t.common.cancel}</Button
			>
			<Button onclick={() => void handleSave()} disabled={!canSave || saving}>
				{saving
					? i18n.t.common.saving
					: isEdit
						? i18n.t.commands.form.saveSubmit
						: i18n.t.commands.form.addSubmit}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
