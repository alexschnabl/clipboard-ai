<!-- lib/components/BackendForm.svelte — add/edit dialog for a single backend -->
<script lang="ts">
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Label } from '$lib/components/shadcn/label/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import RiEyeLine from 'remixicon-svelte/icons/eye-line';
	import RiEyeOffLine from 'remixicon-svelte/icons/eye-off-line';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { KIND_LABELS, type BackendConfig, type BackendKind, type Effort } from '$lib/types';

	/** Fixed, developer-curated set mirroring `config::Effort`'s variants --
	 *  the labels come from the active dictionary (unlike `KIND_LABELS`,
	 *  these are ordinary words worth translating), so this lives inside the
	 *  component as a `$derived`, not as a module-level constant. */
	const EFFORT_VALUES: Effort[] = ['default', 'low', 'medium', 'high', 'xhigh', 'max'];

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
	// `claude-cli`-only fields. Schema defaults (see `Effort`'s doc comment
	// in config.rs): `low` effort, persistence off -- exactly what the CLI
	// adapter used to hardcode, so a new backend behaves the same as it did
	// before these became configurable.
	let effort = $state<Effort>('low');
	let sessionPersistence = $state(false);
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
	/** Only the two HTTP backends take an API key -- `claude-cli` shells out
	 *  to a locally-installed CLI that handles its own auth. A model, though,
	 *  applies to all three: on `claude-cli` it maps to the CLI's `--model`
	 *  flag, the main lever for trading quality for speed (a small model like
	 *  haiku answers faster). */
	const usesKey = $derived(kind === 'openai-compatible' || kind === 'anthropic-api');
	// `satisfies Record<Effort, string>` -- not just `Record<Effort, string>`
	// -- so this stays a plain object literal (usable as `effortLabels[value]`
	// below) while the compiler still rejects a future `Effort` variant left
	// out of this list, the same guarantee `KIND_LABELS` gives `kind`.
	const effortLabels = $derived({
		default: i18n.t.backends.form.effortOptionDefault,
		low: i18n.t.backends.form.effortOptionLow,
		medium: i18n.t.backends.form.effortOptionMedium,
		high: i18n.t.backends.form.effortOptionHigh,
		xhigh: i18n.t.backends.form.effortOptionXhigh,
		max: i18n.t.backends.form.effortOptionMax
	} satisfies Record<Effort, string>);

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
			// `?? 'low'` / `?? false` covers both a brand-new backend and an
			// older one saved before these fields existed -- same fallback
			// the Rust side's `#[serde(default)]` applies on load.
			effort = backend?.effort ?? 'low';
			sessionPersistence = backend?.sessionPersistence ?? false;
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

	async function handleSave(): Promise<void> {
		if (!canSave || saving) return;
		const next: BackendConfig = {
			id: backend?.id ?? crypto.randomUUID(),
			name: name.trim(),
			kind,
			baseUrl: kind === 'openai-compatible' ? baseUrl.trim() || null : null,
			model: model.trim() || null,
			apiKey: usesKey ? apiKey.trim() || null : null,
			timeoutMs,
			// Only `claude-cli` exposes these controls; every other kind
			// always saves the schema default (`low`/`false`) rather than
			// whatever `effort`/`sessionPersistence` happened to be left at
			// from a previous claude-cli edit in this same dialog session.
			effort: kind === 'claude-cli' ? effort : 'low',
			sessionPersistence: kind === 'claude-cli' ? sessionPersistence : false
		};
		saving = true;
		submitError = null;
		const ok = backend ? await configState.updateBackend(next) : await configState.addBackend(next);
		saving = false;
		if (ok) {
			open = false;
		} else {
			submitError = configState.error?.message ?? i18n.t.backends.form.saveFailedFallback;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{isEdit ? i18n.t.backends.form.editTitle : i18n.t.backends.form.addTitle}</Dialog.Title>
			<Dialog.Description>
				{isEdit ? i18n.t.backends.form.editDescription : i18n.t.backends.form.addDescription}
			</Dialog.Description>
		</Dialog.Header>

		<!-- `gap-3` (12px): matches mockup-a-compact.html's `.dialog-body { gap: 12px }`
		     -- see CommandForm.svelte's identical comment for why this is the one
		     spacing lever available here (Dialog.Content's own `p-6 gap-6` is
		     shadcn-owned). -->
		<div class="flex flex-col gap-3">
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
						<!-- F6 (review3-frontend.md): was a third, hardcoded copy of these
						     kind->label pairs -- KIND_LABELS (types.ts) is the one source of
						     truth this same component already uses for the trigger text
						     above; rendering options from it means Rust and TypeScript can
						     no longer silently disagree in only this one place. -->
						{#each Object.entries(KIND_LABELS) as [value, label] (value)}
							<Select.Item {value} {label} />
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			{#if kind === 'openai-compatible'}
				<div class="flex flex-col gap-1.5">
					<Label for="backend-base-url">{i18n.t.backends.form.baseUrlLabel}</Label>
					<Input
						id="backend-base-url"
						bind:value={baseUrl}
						placeholder="http://localhost:11434/v1"
						autocomplete="off"
					/>
				</div>
			{/if}

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

				<div class="flex items-center justify-between gap-3">
					<div class="flex flex-col gap-0.5">
						<Label for="backend-session-persistence"
							>{i18n.t.backends.form.sessionPersistenceLabel}</Label
						>
						<p class="text-xs text-muted-foreground">
							{i18n.t.backends.form.sessionPersistenceHint}
						</p>
					</div>
					<Switch id="backend-session-persistence" bind:checked={sessionPersistence} />
				</div>
			{/if}

			{#if usesKey}
				<div class="flex flex-col gap-1.5">
					<Label for="backend-api-key">
						{i18n.t.backends.form.apiKeyLabel}
						{#if kind === 'openai-compatible'}
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
							class="pr-9"
							autocomplete="off"
						/>
						<!-- F4 (review3-frontend.md): with no size classes of its own, this
						     button rendered at exactly its icon's size-4 (16x16px) -- under
						     WCAG 2.5.8's 24x24 minimum target size. size-6 (24x24) plus
						     flex-centering the icon gives it a proper hit target; `right-1`
						     (was right-2) keeps the icon visually centered in the same spot
						     inside the input's reserved `pr-9`, so the input's own layout is
						     untouched. -->
						<button
							type="button"
							onclick={() => (revealed = !revealed)}
							class="absolute top-1/2 right-1 flex size-6 -translate-y-1/2 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
							aria-label={revealed
								? i18n.t.backends.form.hideApiKey
								: i18n.t.backends.form.showApiKey}
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

		<Dialog.Footer>
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
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
