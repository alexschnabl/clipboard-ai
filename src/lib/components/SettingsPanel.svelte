<script lang="ts">
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import PageHeader from './PageHeader.svelte';
	import DeveloperSettings from './DeveloperSettings.svelte';
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
	<PageHeader title={i18n.t.settings.title} />

	<div class="page-body">
		{#if settingsNotice}
			<Alert.Root variant="destructive">
				<Alert.Title>{i18n.t.settings.saveErrorTitle}</Alert.Title>
				<Alert.Description>{settingsNotice}</Alert.Description>
			</Alert.Root>
		{/if}

		<section>
			<div class="group-title"><span>{i18n.t.settings.groupAppearance}</span></div>
			<div class="list-card">
				<div class="list-row justify-between gap-4">
					<div class="flex flex-col gap-0.5">
						<span>{i18n.t.settings.themeLabel}</span>
						<span class="text-[13px] text-muted-foreground">{i18n.t.settings.themeDescription}</span>
					</div>
					<div
						class="inline-flex shrink-0 gap-0.5 rounded-[9px] bg-muted p-[3px]"
						role="radiogroup"
						aria-label={i18n.t.settings.themeLabel}
					>
						{#each themeOptions as opt (opt.value)}
							{@const active = configState.config.settings.theme === opt.value}
							<button
								type="button"
								role="radio"
								aria-checked={active}
								class="rounded-[7px] px-3 py-1 text-[12.5px] transition-colors duration-150 {active
									? 'bg-card text-foreground shadow-[0_1px_2px_rgb(0_0_0/0.12)]'
									: 'text-muted-foreground hover:text-foreground'}"
								onclick={() => setTheme(opt.value)}
							>
								{opt.label}
							</button>
						{/each}
					</div>
				</div>

				<div class="list-row justify-between gap-4">
					<div class="flex flex-col gap-0.5">
						<span>{i18n.t.settings.languageLabel}</span>
						<span class="text-[13px] text-muted-foreground">{i18n.t.settings.languageDescription}</span>
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
			</div>
		</section>

		<section>
			<div class="group-title"><span>{i18n.t.settings.groupStartup}</span></div>
			<div class="list-card">
				<div class="list-row justify-between gap-4">
					<div class="flex flex-col gap-0.5">
						<span>{i18n.t.settings.autostartLabel}</span>
						<span class="text-[13px] text-muted-foreground">
							{i18n.t.settings.autostartDescription}
							{#if import.meta.env.DEV}
								{i18n.t.settings.autostartDevNoteBefore}<code>npm run tauri build</code
								>{i18n.t.settings.autostartDevNoteAfter}
							{/if}
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
		</section>

		<DeveloperSettings />
	</div>
</div>
