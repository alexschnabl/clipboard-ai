<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import { Input } from '$lib/components/shadcn/input/index.js';
	import { Switch } from '$lib/components/shadcn/switch/index.js';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import CheckIcon from '@lucide/svelte/icons/check';
	import EyeIcon from '@lucide/svelte/icons/eye';
	import EyeOffIcon from '@lucide/svelte/icons/eye-off';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import type { LocalApiSettings, LocalApiStatus } from '$lib/types';

	const MIN_PORT = 1024;
	const MAX_PORT = 65535;
	const CLI_EXAMPLE = 'clipboard-ai --help';

	const apiSettings = $derived<LocalApiSettings>(
		configState.config.settings.localApi ?? { enabled: false, port: null }
	);

	let status = $state<LocalApiStatus | null>(null);
	let pending = $state(false);
	let notice = $state<string | null>(null);
	let revealed = $state(false);
	let copiedKey = $state<string | null>(null);
	let confirmOpen = $state(false);
	let portError = $state<string | null>(null);
	let portDraft = $derived(apiSettings.port === null ? '' : String(apiSettings.port));

	const maskedToken = $derived(
		status?.token ? `${status.token.slice(0, 4)}${'•'.repeat(24)}${status.token.slice(-4)}` : ''
	);

	async function refreshStatus(): Promise<void> {
		try {
			status = await invoke<LocalApiStatus>('local_api_status');
		} catch (e) {
			notice = String(e);
		}
	}

	$effect(() => {
		void apiSettings.enabled;
		void apiSettings.port;
		void refreshStatus();
	});

	async function saveApi(patch: Partial<LocalApiSettings>): Promise<boolean> {
		notice = null;
		pending = true;
		const next: LocalApiSettings = { enabled: apiSettings.enabled, port: apiSettings.port, ...patch };
		const ok = await configState.updateSettings({ ...configState.config.settings, localApi: next });
		pending = false;
		if (!ok) notice = configState.error?.message ?? i18n.t.settings.saveFailedFallback;
		await refreshStatus();
		return ok;
	}

	function setEnabled(checked: boolean): void {
		if (!checked) revealed = false;
		void saveApi({ enabled: checked });
	}

	function commitPort(): void {
		const raw = portDraft.trim();
		if (raw === '') {
			portError = null;
			if (apiSettings.port !== null) void saveApi({ port: null });
			return;
		}
		const port = /^\d{1,5}$/.test(raw) ? Number(raw) : NaN;
		if (!(port >= MIN_PORT && port <= MAX_PORT)) {
			portError = i18n.t.developers.portInvalid(MIN_PORT, MAX_PORT);
			return;
		}
		portError = null;
		portDraft = String(port);
		if (port !== apiSettings.port) void saveApi({ port });
	}

	function handlePortKey(event: KeyboardEvent): void {
		if (event.key === 'Enter') commitPort();
		if (event.key === 'Escape') {
			portError = null;
			portDraft = apiSettings.port === null ? '' : String(apiSettings.port);
		}
	}

	async function copy(text: string, key: string): Promise<void> {
		try {
			await navigator.clipboard.writeText(text);
			copiedKey = key;
			setTimeout(() => {
				if (copiedKey === key) copiedKey = null;
			}, 1500);
		} catch (e) {
			notice = String(e);
		}
	}

	async function regenerate(): Promise<void> {
		confirmOpen = false;
		notice = null;
		try {
			status = await invoke<LocalApiStatus>('local_api_regenerate_token');
		} catch (e) {
			notice = `${i18n.t.developers.regenerateFailed}: ${String(e)}`;
		}
	}
</script>

{#snippet copyButton(text: string, key: string, label: string)}
	<Button variant="ghost" size="icon-sm" aria-label={label} title={label} onclick={() => void copy(text, key)}>
		{#if copiedKey === key}
			<CheckIcon class="size-4 text-success" />
		{:else}
			<CopyIcon class="size-4" />
		{/if}
	</Button>
{/snippet}

<section>
	<div class="group-title"><span>{i18n.t.developers.group}</span></div>

	{#if notice}
		<Alert.Root variant="destructive" class="mb-3">
			<Alert.Title>{i18n.t.settings.saveErrorTitle}</Alert.Title>
			<Alert.Description>{notice}</Alert.Description>
		</Alert.Root>
	{/if}

	<div class="list-card">
		<div class="list-row justify-between gap-4">
			<div class="flex flex-col gap-0.5">
				<span>{i18n.t.developers.apiLabel}</span>
				<span class="text-[13px] text-muted-foreground">{i18n.t.developers.apiDescription}</span>
			</div>
			<Switch
				checked={apiSettings.enabled}
				disabled={pending}
				onCheckedChange={(checked: boolean) => setEnabled(checked)}
				class="shrink-0"
				aria-label={i18n.t.developers.apiLabel}
			/>
		</div>

		{#if apiSettings.enabled}
			<div class="list-row justify-between gap-4">
				<div class="flex min-w-0 flex-col gap-0.5">
					<span>{i18n.t.developers.addressLabel}</span>
					{#if status?.error}
						<span class="text-[13px] text-destructive">{status.error}</span>
					{:else}
						<span class="text-[13px] text-muted-foreground">{i18n.t.developers.addressDescription}</span>
					{/if}
				</div>
				<div class="flex shrink-0 items-center gap-1">
					{#if status?.running && status.url}
						<span class="flex items-center gap-2 rounded-[7px] bg-muted px-2.5 py-1 font-mono text-[12.5px]">
							<span class="size-1.5 rounded-full bg-success" aria-hidden="true"></span>
							<span>{status.url}</span>
							<span class="sr-only">{i18n.t.developers.statusRunning}</span>
						</span>
						{@render copyButton(status.url, 'url', i18n.t.developers.copyAddress)}
					{:else}
						<span class="flex items-center gap-2 text-[13px] text-muted-foreground">
							<span class="size-1.5 rounded-full {status?.error ? 'bg-destructive' : 'bg-muted-foreground/50'}" aria-hidden="true"></span>
							{i18n.t.developers.statusStopped}
						</span>
					{/if}
				</div>
			</div>

			<div class="list-row justify-between gap-4">
				<div class="flex min-w-0 flex-col gap-0.5">
					<label for="local-api-port">{i18n.t.developers.portLabel}</label>
					{#if portError}
						<span class="text-[13px] text-destructive" id="local-api-port-error">{portError}</span>
					{:else}
						<span class="text-[13px] text-muted-foreground">{i18n.t.developers.portDescription}</span>
					{/if}
				</div>
				<Input
					id="local-api-port"
					class="w-28 shrink-0 text-right font-mono tabular-nums placeholder:font-sans"
					inputmode="numeric"
					maxlength={5}
					autocomplete="off"
					placeholder={i18n.t.developers.portPlaceholder}
					disabled={pending}
					aria-invalid={portError ? true : undefined}
					aria-describedby={portError ? 'local-api-port-error' : undefined}
					bind:value={portDraft}
					onblur={commitPort}
					onkeydown={handlePortKey}
				/>
			</div>

			<div class="list-row flex-col items-stretch gap-3">
				<div class="flex items-center justify-between gap-4">
					<div class="flex min-w-0 flex-col gap-0.5">
						<span>{i18n.t.developers.tokenLabel}</span>
						<span class="text-[13px] text-muted-foreground">
							{i18n.t.developers.tokenDescriptionBefore}<code class="font-mono text-[12px]"
								>Authorization: Bearer</code
							>{i18n.t.developers.tokenDescriptionAfter}
						</span>
					</div>
					<div class="flex shrink-0 items-center gap-1">
						{#if status?.token}
							<Button
								variant="ghost"
								size="icon-sm"
								aria-label={revealed ? i18n.t.developers.hideToken : i18n.t.developers.showToken}
								title={revealed ? i18n.t.developers.hideToken : i18n.t.developers.showToken}
								onclick={() => (revealed = !revealed)}
							>
								{#if revealed}
									<EyeOffIcon class="size-4" />
								{:else}
									<EyeIcon class="size-4" />
								{/if}
							</Button>
							{@render copyButton(status.token, 'token', i18n.t.developers.copyToken)}
						{/if}
						<Button variant="outline" size="sm" class="ml-1" onclick={() => (confirmOpen = true)}>
							{i18n.t.developers.regenerate}
						</Button>
					</div>
				</div>
				{#if status?.token}
					<code
						class="rounded-[9px] bg-muted px-3 py-2 font-mono text-[12.5px] break-all select-all"
						aria-label={i18n.t.developers.tokenLabel}
					>
						{revealed ? status.token : maskedToken}
					</code>
				{/if}
			</div>
		{/if}

		<div class="list-row justify-between gap-4">
			<div class="flex min-w-0 flex-col gap-0.5">
				<span>{i18n.t.developers.cliLabel}</span>
				<span class="text-[13px] text-muted-foreground">{i18n.t.developers.cliDescription}</span>
			</div>
			<div class="flex shrink-0 items-center gap-1">
				<code class="rounded-[7px] bg-muted px-2.5 py-1 font-mono text-[12.5px]">{CLI_EXAMPLE}</code>
				{@render copyButton(CLI_EXAMPLE, 'cli', i18n.t.developers.copyCommand)}
			</div>
		</div>
	</div>
</section>

<Dialog.Root bind:open={confirmOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{i18n.t.developers.regenerateTitle}</Dialog.Title>
			<Dialog.Description>{i18n.t.developers.regenerateDescription}</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (confirmOpen = false)}>{i18n.t.common.cancel}</Button>
			<Button onclick={() => void regenerate()}>{i18n.t.developers.regenerateConfirm}</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
