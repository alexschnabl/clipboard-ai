<script lang="ts">
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import TrashIcon from '@lucide/svelte/icons/trash';
	import PageHeader from './PageHeader.svelte';
	import { fade } from 'svelte/transition';
	import { untrack } from 'svelte';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import EyeOffIcon from '@lucide/svelte/icons/eye-off';
	import { configState } from '$lib/state/config.svelte.js';
	import { healthState } from '$lib/state/health.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { KIND_LABELS, type BackendConfig, type BackendKind, type Effort } from '$lib/types';

	const EFFORT_VALUES: Effort[] = ['default', 'low', 'medium', 'high', 'xhigh', 'max'];

	let {
		open = $bindable(false),
		backend = null,
		onDelete
	}: {
		open: boolean;
		backend?: BackendConfig | null;
		onDelete?: () => void;
	} = $props();

	const DEFAULT_TIMEOUT_MS = 120_000;
	const MIN_TIMEOUT_MS = 5000;

	let name = $state('');
	let kind = $state<BackendKind>('openai-compatible');
	let baseUrl = $state('');
	let model = $state('');
	let apiKey = $state('');
	let timeoutMs = $state(DEFAULT_TIMEOUT_MS);
	let effort = $state<Effort>('low');
	let expectsResponse = $state(true);
	let revealed = $state(false);
	let keyStored = $state(false);
	let keyInline = $state(false);
	let keyDirty = $state(false);
	let keyLoaded = $state(false);
	let removeKey = $state(false);
	let revealing = $state(false);
	let revealError = $state<string | null>(null);
	let saving = $state(false);
	let submitError = $state<string | null>(null);

	const isEdit = $derived(backend !== null);
	const timeoutTooLow = $derived(timeoutMs < MIN_TIMEOUT_MS);
	const canSave = $derived(name.trim().length > 0 && !timeoutTooLow);
	const usesKey = $derived(
		kind === 'openai-compatible' || kind === 'anthropic-api' || kind === 'http'
	);
	const usesBaseUrl = $derived(kind === 'openai-compatible' || kind === 'http');
	const effortLabels = $derived({
		default: i18n.t.backends.form.effortOptionDefault,
		low: i18n.t.backends.form.effortOptionLow,
		medium: i18n.t.backends.form.effortOptionMedium,
		high: i18n.t.backends.form.effortOptionHigh,
		xhigh: i18n.t.backends.form.effortOptionXhigh,
		max: i18n.t.backends.form.effortOptionMax
	} satisfies Record<Effort, string>);

	let wasOpen = false;
	$effect(() => {
		if (open) {
			wasOpen = true;
			name = backend?.name ?? '';
			kind = backend?.kind ?? 'openai-compatible';
			baseUrl = backend?.baseUrl ?? '';
			model = backend?.model ?? '';
			apiKey = '';
			const id = backend?.id;
			keyStored = id ? untrack(() => configState.hasStoredKey(id)) : false;
			keyInline = id ? untrack(() => configState.hasInlineKey(id)) : false;
			keyDirty = false;
			keyLoaded = false;
			removeKey = false;
			revealing = false;
			revealError = null;
			timeoutMs = backend?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
			effort = backend?.effort ?? 'low';
			expectsResponse = backend?.expectsResponse ?? true;
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

	function handleEffortChange(value: string | undefined): void {
		if (typeof value !== 'string') return;
		effort = value as Effort;
	}

	function handleTimeoutInput(e: Event & { currentTarget: HTMLInputElement }): void {
		const v = Number(e.currentTarget.value);
		timeoutMs = Number.isFinite(v) ? v : DEFAULT_TIMEOUT_MS;
	}

	async function toggleReveal(): Promise<void> {
		revealError = null;
		if (revealed) {
			revealed = false;
			return;
		}
		if (backend && keyStored && !keyLoaded && !keyDirty && !removeKey) {
			revealing = true;
			try {
				apiKey = (await configState.revealApiKey(backend.id)) ?? '';
				keyLoaded = true;
			} catch {
				revealError = i18n.t.backends.form.apiKeyRevealFailed;
				return;
			} finally {
				revealing = false;
			}
		}
		revealed = true;
	}

	function handleKeyInput(): void {
		keyDirty = true;
		removeKey = false;
	}

	function markKeyForRemoval(): void {
		removeKey = true;
		keyDirty = false;
		keyLoaded = false;
		revealed = false;
		apiKey = '';
	}

	function keyChange(): string | null | undefined {
		if (!usesKey) return undefined;
		if (removeKey) return null;
		if (!keyDirty) return undefined;
		const trimmed = apiKey.trim();
		return trimmed.length > 0 ? trimmed : undefined;
	}

	async function handleSave(): Promise<void> {
		if (!canSave || saving) return;
		const next: BackendConfig = {
			id: backend?.id ?? crypto.randomUUID(),
			name: name.trim(),
			kind,
			baseUrl: usesBaseUrl ? baseUrl.trim() || null : null,
			model: kind === 'http' ? null : model.trim() || null,
			apiKey: null,
			timeoutMs,
			effort: kind === 'claude-cli' ? effort : 'low',
			expectsResponse: kind === 'http' ? expectsResponse : true
		};
		saving = true;
		submitError = null;
		const change = keyChange();
		const ok = backend
			? await configState.updateBackend(next, change)
			: await configState.addBackend(next, change);
		saving = false;
		if (ok) {
			open = false;
			healthState.forget(next.id);
			void healthState.check(next.id);
		} else {
			submitError = configState.error?.message ?? i18n.t.backends.form.saveFailedFallback;
		}
	}

	function onWindowKeydown(event: KeyboardEvent): void {
		if (!open || saving || event.key !== 'Escape' || event.defaultPrevented) return;
		if (document.querySelector('[data-slot=select-content], [data-slot=dialog-content]')) return;
		open = false;
	}
</script>

<svelte:window onkeydown={onWindowKeydown} />

{#if open}
	<div class="flex h-full min-h-0 flex-col" in:fade={{ duration: 120 }}>
		<PageHeader
			title={isEdit ? (backend?.name ?? '') : i18n.t.backends.form.addTitle}
			onBack={() => (open = false)}
		/>

		<div class="page-body">
			<div class="list-card flex flex-col gap-4 p-5">
				<div class="flex flex-col gap-1.5">
					<Label for="backend-name">{i18n.t.backends.form.nameLabel}</Label>
					<Input
						id="backend-name"
						bind:value={name}
						placeholder={i18n.t.backends.form.namePlaceholder}
						autocomplete="off"
					/>
				</div>

				<div class="flex flex-col gap-1.5">
					<Label for="backend-kind">{i18n.t.backends.form.kindLabel}</Label>
					<Select.Root type="single" value={kind} onValueChange={handleKindChange}>
						<Select.Trigger id="backend-kind" class="w-full">
							{KIND_LABELS[kind]}
						</Select.Trigger>
						<Select.Content>
							{#each Object.entries(KIND_LABELS) as [value, label] (value)}
								<Select.Item {value} {label} />
							{/each}
						</Select.Content>
					</Select.Root>
				</div>

				{#if usesBaseUrl}
					<div class="flex flex-col gap-1.5">
						<Label for="backend-base-url">{i18n.t.backends.form.baseUrlLabel}</Label>
						<Input
							id="backend-base-url"
							bind:value={baseUrl}
							placeholder={kind === 'http'
								? 'http://localhost:8080/transform'
								: 'http://localhost:11434/v1'}
							autocomplete="off"
						/>
						{#if kind === 'http'}
							<p class="text-xs text-muted-foreground">{i18n.t.backends.form.httpUrlHint}</p>
						{/if}
					</div>
				{/if}

				{#if kind !== 'http'}
					<div class="flex flex-col gap-1.5">
						<Label for="backend-model">{i18n.t.backends.form.modelLabel}</Label>
						<Input
							id="backend-model"
							bind:value={model}
							placeholder={kind === 'openai-compatible'
								? i18n.t.backends.form.modelPlaceholderOpenAiCompatible
								: kind === 'claude-cli'
									? i18n.t.backends.form.modelPlaceholderClaudeCli
									: i18n.t.backends.form.modelPlaceholderOther}
							autocomplete="off"
						/>
						{#if kind === 'claude-cli'}
							<p class="text-xs text-muted-foreground">{i18n.t.backends.form.modelHintClaudeCli}</p>
						{/if}
					</div>
				{/if}

				{#if kind === 'claude-cli'}
					<div class="flex flex-col gap-1.5">
						<Label for="backend-effort">{i18n.t.backends.form.effortLabel}</Label>
						<Select.Root type="single" value={effort} onValueChange={handleEffortChange}>
							<Select.Trigger id="backend-effort" class="w-full">
								{effortLabels[effort]}
							</Select.Trigger>
							<Select.Content>
								{#each EFFORT_VALUES as value (value)}
									<Select.Item {value} label={effortLabels[value]} />
								{/each}
							</Select.Content>
						</Select.Root>
						<p class="text-xs text-muted-foreground">{i18n.t.backends.form.effortHint}</p>
					</div>
				{/if}

				{#if kind === 'http'}
					<div class="flex items-center justify-between gap-3">
						<div class="flex flex-col gap-0.5">
							<Label for="backend-expect-response">{i18n.t.backends.form.expectResponseLabel}</Label>
							<p class="text-xs text-muted-foreground">
								{i18n.t.backends.form.expectResponseHint}
							</p>
						</div>
						<Switch id="backend-expect-response" bind:checked={expectsResponse} />
					</div>
				{/if}

				{#if usesKey}
					<div class="flex flex-col gap-1.5">
						<Label for="backend-api-key">
							{i18n.t.backends.form.apiKeyLabel}
							{#if kind === 'openai-compatible' || kind === 'http'}
								<span class="font-normal text-muted-foreground"
									>{i18n.t.backends.form.apiKeyOptional}</span
								>
							{/if}
						</Label>
						<div class="relative">
							<Input
								id="backend-api-key"
								type={revealed ? 'text' : 'password'}
								bind:value={apiKey}
								oninput={handleKeyInput}
								placeholder={keyStored && !keyInline && !keyDirty && !keyLoaded && !removeKey
									? i18n.t.backends.form.apiKeyStoredPlaceholder
									: ''}
								class="pr-9"
								autocomplete="off"
								spellcheck="false"
							/>
							<button
								type="button"
								onclick={() => void toggleReveal()}
								disabled={revealing}
								class="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
								aria-label={revealed
									? i18n.t.backends.form.hideApiKey
									: i18n.t.backends.form.showApiKey}
							>
								{#if revealed}
									<EyeOffIcon class="size-4" />
								{:else}
									<EyeIcon class="size-4" />
								{/if}
							</button>
						</div>
						{#if revealError}
							<p class="text-xs text-destructive">{revealError}</p>
						{:else if removeKey}
							<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
								{i18n.t.backends.form.apiKeyWillBeRemoved}
								<button
									type="button"
									class="text-foreground underline-offset-2 hover:underline"
									onclick={() => (removeKey = false)}>{i18n.t.backends.form.apiKeyUndo}</button
								>
							</p>
						{:else}
							<p class="flex items-center gap-1.5 text-xs text-muted-foreground">
								{keyInline && !keyDirty
									? i18n.t.backends.form.apiKeyInline
									: i18n.t.backends.form.apiKeyHint}
								{#if keyStored}
									<button
										type="button"
										class="text-foreground underline-offset-2 hover:underline"
										onclick={markKeyForRemoval}>{i18n.t.backends.form.apiKeyRemove}</button
									>
								{/if}
							</p>
						{/if}
					</div>
				{/if}

				<div class="flex flex-col gap-1.5">
					<Label for="backend-timeout">{i18n.t.backends.form.timeoutLabel}</Label>
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
							{i18n.t.backends.form.timeoutHint(DEFAULT_TIMEOUT_MS.toLocaleString())}
						</p>
					{/if}
					{#if timeoutTooLow}
						<p class="text-xs text-destructive">
							{i18n.t.backends.form.timeoutTooLow(MIN_TIMEOUT_MS)}
						</p>
					{/if}
				</div>
			</div>

			{#if submitError}
				<Alert.Root variant="destructive">
					<Alert.Title>{i18n.t.backends.form.saveError}</Alert.Title>
					<Alert.Description>{submitError}</Alert.Description>
				</Alert.Root>
			{/if}

			<div class="flex items-center gap-2">
				{#if isEdit && onDelete}
					<Button variant="ghost" class="text-destructive hover:text-destructive" onclick={onDelete}>
						<TrashIcon class="size-4" />
						{i18n.t.backends.form.deleteSubmit}
					</Button>
				{/if}
				<div class="flex-1"></div>
				<Button variant="outline" onclick={() => (open = false)} disabled={saving}
					>{i18n.t.common.cancel}</Button
				>
				<Button onclick={() => void handleSave()} disabled={!canSave || saving}>
					{saving
						? i18n.t.common.saving
						: isEdit
							? i18n.t.backends.form.saveSubmit
							: i18n.t.backends.form.addSubmit}
				</Button>
			</div>
		</div>
	</div>
{/if}
