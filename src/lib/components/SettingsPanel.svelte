<!-- lib/components/SettingsPanel.svelte — theme, language and autostart.
     Three first-class settings, each persisted the instant it changes
     through configState.updateSettings (the same commit-and-rollback path
     every backend/command edit already uses) -- there is no Save button
     here, same as everywhere else in this app. -->
<script lang="ts">
	import * as Select from '$lib/components/shadcn/select/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { Language, Settings, Theme } from '$lib/types';

	/** Rebuilt on every read via `$derived`, not a plain module-level const:
	 *  the labels come from `i18n.t`, which is itself reactive (see
	 *  i18n.svelte.ts), so this has to re-run whenever the user changes the
	 *  language -- a plain const would freeze the labels at whatever locale
	 *  was active when the component first mounted. */
	const themeOptions = $derived<{ value: Theme; label: string }[]>([
		{ value: 'system', label: i18n.t.settings.themeSystem },
		{ value: 'light', label: i18n.t.settings.themeLight },
		{ value: 'dark', label: i18n.t.settings.themeDark }
	]);

	const languageOptions = $derived<{ value: Language; label: string }[]>([
		{ value: 'system', label: i18n.t.settings.languageSystem },
		{ value: 'en', label: i18n.t.settings.languageEnglish },
		{ value: 'de', label: i18n.t.settings.languageGerman }
	]);

	/** Set right after any `updateSettings` rejection or refusal -- one shared
	 *  slot for all three settings rather than a field each, since only one
	 *  control is ever mid-change at a time in this UI. Not in `$lib/i18n`'s
	 *  dictionary (it covers the static labels below, not save-failure text,
	 *  which is dynamic and often Rust-origin) -- stays a plain literal, same
	 *  treatment the rest of this app gives `configState.error`. */
	let settingsNotice = $state<string | null>(null);
	let autostartPending = $state(false);

	const languageLabel = $derived(
		languageOptions.find((o) => o.value === configState.config.settings.language)?.label ??
			i18n.t.settings.languageSystem
	);

	/** Every setter below funnels through here: spread the one field that
	 *  changed over the current `Settings` and hand the whole object to
	 *  `configState.updateSettings`, matching `updateBackend`/`updateCommand`'s
	 *  "hand over the intended next value" shape rather than a partial patch.
	 *  A rejected commit already rolls `config.settings` back on its own (see
	 *  `ConfigState.commit`) -- this only needs to surface *why*. */
	async function applySettings(patch: Partial<Settings>): Promise<boolean> {
		settingsNotice = null;
		const ok = await configState.updateSettings({ ...configState.config.settings, ...patch });
		if (!ok) settingsNotice = configState.error?.message ?? 'Could not save the change.';
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

	/** Enabling this can be refused by the backend even on an otherwise-`ok`
	 *  save: a debug build's `settings::apply_autostart` deliberately declines
	 *  to touch the registry at all (see that function's doc comment --
	 *  `auto_launch` would register this machine's own
	 *  `target\debug\clipboard-ai-app.exe`, not the installed app) and reports
	 *  that refusal as a `Problem` rather than failing the whole config write.
	 *  Without checking for it here, the switch would flip on, the write
	 *  would report success, and the OS-level registration would silently
	 *  never happen -- exactly the "appears to work and silently doesn't"
	 *  the task brief calls out. */
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

<div class="flex flex-col gap-4">
	<h2 class="text-sm font-medium">{i18n.t.settings.title}</h2>

	{#if settingsNotice}
		<Alert.Root variant="destructive">
			<Alert.Title>Couldn’t save that change</Alert.Title>
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
					{i18n.t.settings.autostartDescription} Refused in development builds — it would register
					the debug executable to run at login instead of your installed app; test this with an
					installed (<code>npm run tauri build</code>) build instead.
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
