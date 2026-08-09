<script lang="ts">
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { registry, type LanguageChoice } from '$lib/i18n/registry';
	import type { Language, Settings, Theme } from '$lib/types';

	const themeOptions = $derived<{ value: Theme; label: string }[]>([
		{ value: 'system', label: i18n.t.settings.themeSystem },
		{ value: 'light', label: i18n.t.settings.themeLight },
		{ value: 'dark', label: i18n.t.settings.themeDark }
	]);

	const languageOptions = $derived<{ value: LanguageChoice; label: string }[]>([
		{ value: 'system', label: i18n.t.settings.languageSystem },
		...Object.entries(registry).map(([code, entry]) => ({
			value: code as LanguageChoice,
			label: entry.nativeName
		}))
	]);

	let settingsNotice = $state<string | null>(null);
	let autostartPending = $state(false);

	const languageLabel = $derived(
		languageOptions.find((o) => o.value === configState.config.settings.language)?.label ??
			i18n.t.settings.languageSystem
	);

	async function applySettings(patch: Partial<Settings>): Promise<boolean> {
		settingsNotice = null;
		const ok = await configState.updateSettings({ ...configState.config.settings, ...patch });
		if (!ok) settingsNotice = configState.error?.message ?? i18n.t.settings.saveFailedFallback;
		return ok;
	}

	function setTheme(theme: Theme): void {
		if (configState.config.settings.theme === theme) return;
		void applySettings({ theme });
	}

	function handleLanguageChange(value: string | undefined): void {
		if (typeof value !== 'string' || value === configState.config.settings.language) return;
		void applySettings({ language: value as Language });
	}

	async function setAutostart(checked: boolean): Promise<void> {
		autostartPending = true;
		const ok = await applySettings({ autostart: checked });
		autostartPending = false;
		if (ok) {
			const refusal = configState.problems.find((p) => /autostart/i.test(p.message));
			if (refusal) settingsNotice = refusal.message;
		}
	}
</script>

<div class="flex h-full min-h-0 flex-col">
	<div class="flex shrink-0 items-center gap-2 border-b px-4 py-2.5">
		<h2 class="text-[12.5px] font-bold">{i18n.t.settings.title}</h2>
		<div class="flex-1"></div>
	</div>

	<div class="min-h-0 flex-1 overflow-y-auto p-4">
		{#if settingsNotice}
			<Alert.Root variant="destructive" class="mb-3">
				<Alert.Title>{i18n.t.settings.saveErrorTitle}</Alert.Title>
				<Alert.Description>{settingsNotice}</Alert.Description>
			</Alert.Root>
		{/if}

		<div class="flex flex-col">
			<div class="flex items-center justify-between gap-4 border-b py-3">
				<div class="flex flex-col gap-0.5">
					<span class="text-sm font-medium">{i18n.t.settings.themeLabel}</span>
					<span class="text-xs text-muted-foreground">{i18n.t.settings.themeDescription}</span>
				</div>
				<div
					class="inline-flex shrink-0 gap-0.5 rounded-md bg-muted p-0.5"
					role="radiogroup"
					aria-label={i18n.t.settings.themeLabel}
				>
					{#each themeOptions as opt (opt.value)}
						{@const active = configState.config.settings.theme === opt.value}
						<button
							type="button"
							role="radio"
							aria-checked={active}
							class="rounded-sm px-2.5 py-1 text-xs font-medium transition-colors {active
								? 'bg-background text-foreground shadow-sm'
								: 'text-muted-foreground hover:text-foreground'}"
							onclick={() => setTheme(opt.value)}
						>
							{opt.label}
						</button>
					{/each}
				</div>
			</div>

			<div class="flex items-center justify-between gap-4 border-b py-3">
				<div class="flex flex-col gap-0.5">
					<span class="text-sm font-medium">{i18n.t.settings.languageLabel}</span>
					<span class="text-xs text-muted-foreground">{i18n.t.settings.languageDescription}</span>
				</div>
				<Select.Root
					type="single"
					value={configState.config.settings.language}
					onValueChange={handleLanguageChange}
				>
					<Select.Trigger class="w-32 shrink-0">{languageLabel}</Select.Trigger>
					<Select.Content>
						{#each languageOptions as opt (opt.value)}
							<Select.Item value={opt.value} label={opt.label} />
						{/each}
					</Select.Content>
				</Select.Root>
			</div>

			<div class="flex items-center justify-between gap-4 py-3">
				<div class="flex flex-col gap-0.5">
					<span class="text-sm font-medium">{i18n.t.settings.autostartLabel}</span>
					<span class="text-xs text-muted-foreground">
						{i18n.t.settings.autostartDescription}
						{i18n.t.settings.autostartDevNoteBefore}<code>npm run tauri build</code
						>{i18n.t.settings.autostartDevNoteAfter}
					</span>
				</div>
				<Switch
					checked={configState.config.settings.autostart}
					disabled={autostartPending}
					onCheckedChange={(checked: boolean) => void setAutostart(checked)}
					class="shrink-0"
					aria-label={i18n.t.settings.autostartLabel}
				/>
			</div>
		</div>
	</div>
</div>
