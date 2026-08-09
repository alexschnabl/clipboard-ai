<script lang="ts">
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import SettingsSidebar, { type SectionId } from '$lib/components/SettingsSidebar.svelte';
	import BackendList from '$lib/components/BackendList.svelte';
	import CommandList from '$lib/components/CommandList.svelte';
	import HistoryPanel from '$lib/components/HistoryPanel.svelte';
	import SettingsPanel from '$lib/components/SettingsPanel.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { activity } from '$lib/state/activity.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	let activeSection = $state<SectionId>('commands');

	onMount(() => {
		void configState.load();

		const unlistenHidden = listen('settings://window-hidden', () => {
			configState.revision++;
		});

		const unlistenCycle = listen<boolean>('cycle://state', (event) => {
			activity.running = event.payload;
		});
		const unlistenHiddenReset = listen('settings://window-hidden', () => {
			activity.running = false;
		});

		return () => {
			void unlistenHidden.then((f) => f());
			void unlistenCycle.then((f) => f());
			void unlistenHiddenReset.then((f) => f());
		};
	});
</script>

<div class="flex h-screen min-h-0 overflow-hidden">
	<SettingsSidebar
		active={activeSection}
		onNavigate={(id) => (activeSection = id)}
		commandCount={configState.config.commands.length}
		backendCount={configState.config.backends.length}
	/>

	<div class="flex min-w-0 flex-1 flex-col overflow-hidden">
		{#if configState.loadError || configState.error || configState.problems.length > 0}
			<div class="flex max-h-48 shrink-0 flex-col gap-2 overflow-y-auto border-b px-4 py-3">
				{#if configState.loadError}
					<Alert.Root variant="destructive">
						<Alert.Title>{i18n.t.app.loadErrorTitle}</Alert.Title>
						<Alert.Description>
							<p>{configState.loadError.reason}</p>
							{#if configState.loadError.preservedPath}
								<p>
									{i18n.t.app.loadErrorPreservedBefore}
									<code>{configState.loadError.preservedPath}</code>. {i18n.t.app
										.loadErrorPreservedAfter}
								</p>
							{:else}
								<p>{i18n.t.app.loadErrorNotPreserved}</p>
							{/if}
							<p>{i18n.t.app.loadErrorFallbackNotice}</p>
						</Alert.Description>
					</Alert.Root>
				{/if}

				{#if configState.error}
					<Alert.Root variant="destructive">
						<Alert.Title>
							{configState.error.source === 'load'
								? i18n.t.app.errorLoadTitle
								: i18n.t.app.errorSaveTitle}
						</Alert.Title>
						<Alert.Description>{configState.error.message}</Alert.Description>
					</Alert.Root>
				{/if}

				{#if configState.problems.length > 0}
					<Alert.Root>
						<Alert.Title>{i18n.t.app.problemsTitle}</Alert.Title>
						<Alert.Description>
							<ul class="list-disc pl-4">
								{#each configState.problems as problem, i (i)}
									<li>{problem.message}</li>
								{/each}
							</ul>
						</Alert.Description>
					</Alert.Root>
				{/if}
			</div>
		{/if}

		{#if configState.loading}
			<p class="p-4 text-sm text-muted-foreground">{i18n.t.app.loading}</p>
		{:else}
			<div class="flex min-h-0 flex-1 flex-col overflow-hidden">
				{#key configState.revision}
					{#if activeSection === 'commands'}
						<CommandList />
					{:else if activeSection === 'backends'}
						<BackendList />
					{:else if activeSection === 'history'}
						<HistoryPanel />
					{:else}
						<SettingsPanel />
					{/if}
				{/key}
			</div>
		{/if}
	</div>
</div>
