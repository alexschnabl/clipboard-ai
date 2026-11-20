<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import { Textarea } from '$lib/components/shadcn/textarea/index.js';
	import LoaderIcon from '@lucide/svelte/icons/loader-circle';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { defaultWriterBackend, editorErrorKey, isAiBackend, toEditorError } from '$lib/editor';
	import type { EditorError } from '$lib/types';

	let {
		open = $bindable(false),
		currentPrompt,
		commandBackendId,
		onWritten
	}: {
		open: boolean;
		currentPrompt: string;
		commandBackendId: string;
		onWritten: (prompt: string, backendName: string) => void;
	} = $props();

	let description = $state('');
	let backendId = $state('');
	let working = $state(false);
	let error = $state<EditorError | null>(null);
	let generation = 0;
	let descriptionRef = $state<HTMLTextAreaElement | null>(null);

	const aiBackends = $derived(configState.config.backends.filter(isAiBackend));
	const backendName = $derived(aiBackends.find((b) => b.id === backendId)?.name ?? '');

	let wasOpen = false;
	$effect(() => {
		if (open && !wasOpen) {
			backendId = defaultWriterBackend(configState.config.backends, commandBackendId) ?? '';
			error = null;
		}
		if (!open && wasOpen) {
			generation++;
			working = false;
		}
		wasOpen = open;
	});

	const canSubmit = $derived(description.trim().length > 0 && backendId.length > 0 && !working);

	async function submit(): Promise<void> {
		if (!canSubmit) return;
		const mine = ++generation;
		working = true;
		error = null;
		const name = backendName;
		try {
			const prompt = await invoke<string>('write_prompt', {
				backendId,
				description: description.trim(),
				currentPrompt
			});
			if (mine !== generation) return;
			onWritten(prompt, name);
			description = '';
			open = false;
		} catch (e) {
			if (mine !== generation) return;
			error = toEditorError(e);
		} finally {
			if (mine === generation) working = false;
		}
	}

	function onKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			void submit();
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content
		class="sm:max-w-lg"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			descriptionRef?.focus();
		}}
	>
		<Dialog.Header>
			<Dialog.Title>{i18n.t.commands.writer.title}</Dialog.Title>
			<Dialog.Description>{i18n.t.commands.writer.description}</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-1.5">
			<Label for="writer-description">{i18n.t.commands.writer.label}</Label>
			<Textarea
				id="writer-description"
				bind:ref={descriptionRef}
				bind:value={description}
				onkeydown={onKeydown}
				rows={3}
				placeholder={i18n.t.commands.writer.placeholder}
				class="max-h-48 overflow-y-auto"
				disabled={working}
			/>
			{#if currentPrompt.trim()}
				<p class="text-xs text-muted-foreground">{i18n.t.commands.writer.reviseHint}</p>
			{/if}
		</div>

		<div class="flex flex-col gap-1.5">
			<Label for="writer-backend">{i18n.t.commands.writer.backendLabel}</Label>
			<Select.Root type="single" bind:value={backendId} disabled={working}>
				<Select.Trigger id="writer-backend" class="w-full">
					<span class="truncate">{backendName}</span>
				</Select.Trigger>
				<Select.Content>
					{#each aiBackends as backend (backend.id)}
						<Select.Item value={backend.id} label={backend.name} />
					{/each}
				</Select.Content>
			</Select.Root>
		</div>

		{#if error}
			{@const key = editorErrorKey(error.reason)}
			<div class="flex items-start gap-2.5 rounded-[10px] border px-3 py-2.5 text-[13px]" role="alert">
				<span class="mt-[7px] size-2 shrink-0 rounded-full bg-destructive"></span>
				<div class="min-w-0">
					<p>{i18n.t.commands.writer.errorTitle}. {i18n.t.commands.tryIt.errors[key]}</p>
					{#if error.detail}
						<p class="mt-1 text-xs break-words text-muted-foreground">{error.detail}</p>
					{/if}
				</div>
			</div>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)}>{i18n.t.common.cancel}</Button>
			<Button onclick={() => void submit()} disabled={!canSubmit}>
				{#if working}
					<LoaderIcon class="size-4 animate-spin" />
					{i18n.t.commands.writer.working}
				{:else}
					{i18n.t.commands.writer.submit}
				{/if}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
