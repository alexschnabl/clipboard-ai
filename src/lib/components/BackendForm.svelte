<!-- lib/components/BackendForm.svelte — add/edit dialog for a single backend -->
<script lang="ts">
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import RiEyeLine from 'remixicon-svelte/icons/eye-line';
	import RiEyeOffLine from 'remixicon-svelte/icons/eye-off-line';
	import { configState } from '$lib/state/config.svelte.js';
	import { KIND_LABELS, type BackendConfig, type BackendKind } from '$lib/types';

	let {
		open = $bindable(false),
		backend = null
	}: {
		open: boolean;
		/** The backend being edited, or null when adding a new one. */
		backend?: BackendConfig | null;
	} = $props();

	const DEFAULT_TIMEOUT_MS = 120_000;
	/**
	 * Must match `MIN_TIMEOUT_MS` in `src-tauri/src/config.rs`, which is the
	 * authority — Rust rejects the save regardless. This copy exists only so
	 * the form can say so before a round trip, not as a second rule.
	 */
	const MIN_TIMEOUT_MS = 5000;

	let name = $state('');
	let kind = $state<BackendKind>('openai-compatible');
	let baseUrl = $state('');
	let model = $state('');
	let apiKey = $state('');
	let timeoutMs = $state(DEFAULT_TIMEOUT_MS);
	let revealed = $state(false);
	let saving = $state(false);
	/** Set from `configState.error` when `save()` (via `commit`) rejects this
	 *  dialog's edit -- shown inline, dialog left open, so the user can fix
	 *  the field and retry (see fix3 report). Reset on open, not just on a
	 *  fresh attempt, so a stale error from a *previous* dialog session never
	 *  bleeds into one that hasn't tried to save yet. */
	let submitError = $state<string | null>(null);

	const isEdit = $derived(backend !== null);
	// `min={MIN_TIMEOUT_MS}` on the <Input> below is HTML-only -- it doesn't stop a
	// typed "1" from reaching `timeoutMs`. Enforce the floor here too, so
	// the form is genuinely unsaveable below it rather than just visually
	// suggesting one.
	const timeoutTooLow = $derived(timeoutMs < MIN_TIMEOUT_MS);
	const canSave = $derived(name.trim().length > 0 && !timeoutTooLow);
	/** `claude-cli` needs neither -- it shells out to a locally-installed CLI
	 *  that handles its own auth and model choice. Used both here (what to
	 *  send on save) and in the template (which fields to show). */
	const usesModelAndKey = $derived(kind === 'openai-compatible' || kind === 'anthropic-api');

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
			name = backend?.name ?? '';
			kind = backend?.kind ?? 'openai-compatible';
			baseUrl = backend?.baseUrl ?? '';
			model = backend?.model ?? '';
			apiKey = backend?.apiKey ?? '';
			timeoutMs = backend?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
			revealed = false;
			submitError = null;
		} else if (wasOpen) {
			wasOpen = false;
			configState.error = null;
		}
	});

	function handleKindChange(value: string | undefined): void {
		if (typeof value !== 'string') return;
		kind = value as BackendKind;
	}

	function handleTimeoutInput(e: Event & { currentTarget: HTMLInputElement }): void {
		const v = Number(e.currentTarget.value);
		timeoutMs = Number.isFinite(v) ? v : DEFAULT_TIMEOUT_MS;
	}

	async function handleSave(): Promise<void> {
		if (!canSave || saving) return;
		const next: BackendConfig = {
			id: backend?.id ?? crypto.randomUUID(),
			name: name.trim(),
			kind,
			baseUrl: kind === 'openai-compatible' ? baseUrl.trim() || null : null,
			model: usesModelAndKey ? model.trim() || null : null,
			apiKey: usesModelAndKey ? apiKey.trim() || null : null,
			timeoutMs
		};
		saving = true;
		submitError = null;
		const ok = backend ? await configState.updateBackend(next) : await configState.addBackend(next);
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
			<Dialog.Title>{isEdit ? 'Edit backend' : 'Add backend'}</Dialog.Title>
			<Dialog.Description>
				{isEdit ? 'Update this backend’s connection details.' : 'Configure a new AI backend.'}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-4 py-2">
			<div class="flex flex-col gap-1.5">
				<Label for="backend-name">Name</Label>
				<Input id="backend-name" bind:value={name} placeholder="Local Ollama" autocomplete="off" />
			</div>

			<div class="flex flex-col gap-1.5">
				<Label for="backend-kind">Kind</Label>
				<Select.Root type="single" value={kind} onValueChange={handleKindChange}>
					<Select.Trigger id="backend-kind" class="w-full">
						{KIND_LABELS[kind]}
					</Select.Trigger>
					<Select.Content>
						<Select.Item value="openai-compatible" label="OpenAI-compatible" />
						<Select.Item value="anthropic-api" label="Anthropic API" />
						<Select.Item value="claude-cli" label="Claude CLI" />
					</Select.Content>
				</Select.Root>
			</div>

			{#if kind === 'openai-compatible'}
				<div class="flex flex-col gap-1.5">
					<Label for="backend-base-url">Base URL</Label>
					<Input
						id="backend-base-url"
						bind:value={baseUrl}
						placeholder="http://localhost:11434/v1"
						autocomplete="off"
					/>
				</div>
			{/if}

			{#if usesModelAndKey}
				<div class="flex flex-col gap-1.5">
					<Label for="backend-model">Model</Label>
					<Input
						id="backend-model"
						bind:value={model}
						placeholder={kind === 'openai-compatible' ? 'qwen2.5:32b-instruct' : 'claude-opus-5'}
						autocomplete="off"
					/>
				</div>
				<div class="flex flex-col gap-1.5">
					<Label for="backend-api-key">
						API key
						{#if kind === 'openai-compatible'}
							<span class="font-normal text-muted-foreground">(optional)</span>
						{/if}
					</Label>
					<div class="relative">
						<Input
							id="backend-api-key"
							type={revealed ? 'text' : 'password'}
							bind:value={apiKey}
							class="pr-9"
							autocomplete="off"
						/>
						<button
							type="button"
							onclick={() => (revealed = !revealed)}
							class="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
							aria-label={revealed ? 'Hide API key' : 'Show API key'}
						>
							{#if revealed}
								<RiEyeOffLine class="size-4" />
							{:else}
								<RiEyeLine class="size-4" />
							{/if}
						</button>
					</div>
				</div>
			{/if}

			<div class="flex flex-col gap-1.5">
				<Label for="backend-timeout">Timeout (ms)</Label>
				<Input
					id="backend-timeout"
					type="number"
					min={MIN_TIMEOUT_MS}
					step="1000"
					value={timeoutMs}
					oninput={handleTimeoutInput}
					aria-invalid={timeoutTooLow}
				/>
				{#if kind === 'openai-compatible'}
					<p class="text-xs text-muted-foreground">
						A cold local model can take ~16,000 ms on its first request of the day — the
						{DEFAULT_TIMEOUT_MS.toLocaleString()} ms default avoids a false timeout then.
					</p>
				{/if}
				{#if timeoutTooLow}
					<p class="text-xs text-destructive">
						Timeout must be at least {MIN_TIMEOUT_MS} ms.
					</p>
				{/if}
			</div>
		</div>

		{#if submitError}
			<Alert.Root variant="destructive">
				<Alert.Title>Couldn’t save backend</Alert.Title>
				<Alert.Description>{submitError}</Alert.Description>
			</Alert.Root>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (open = false)} disabled={saving}>Cancel</Button>
			<Button onclick={() => void handleSave()} disabled={!canSave || saving}>
				{saving ? 'Saving…' : isEdit ? 'Save changes' : 'Add backend'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
