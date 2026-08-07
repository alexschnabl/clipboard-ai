<!-- routes/+page.svelte — settings window: Backends and Commands tabs -->
<script lang="ts">
	import * as Tabs from '$lib/components/shadcn/tabs/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import BackendList from '$lib/components/BackendList.svelte';
	import CommandList from '$lib/components/CommandList.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	onMount(() => {
		void configState.load();
	});

	// The settings window is hidden, never destroyed (see `hide_and_reapply`
	// in commands.rs), so anything transient a running Svelte tree holds --
	// a `HotkeyRecorder` mid-recording, an open `BackendForm`/`CommandForm`
	// dialog with a stale draft -- would otherwise survive straight through
	// a hide/reshow cycle. Rust emits this event every time it actually
	// hides the window; bumping `configState.revision` here, consumed by the
	// `{#key}` blocks below, remounts the tab bodies so all of that resets.
	// Listening for this event specifically, rather than
	// `document.visibilitychange`, is deliberate -- WebView2 firing that on
	// a native window hide is not guaranteed.
	onMount(() => {
		const unlisten = listen('settings://window-hidden', () => {
			configState.revision++;
		});
		return () => {
			void unlisten.then((f) => f());
		};
	});
</script>

<div class="flex flex-col gap-6 p-8">
	<div>
		<h1 class="text-2xl font-semibold">Clipboard AI</h1>
		<p class="text-sm text-muted-foreground">Configure backends and commands.</p>
	</div>

	{#if configState.loadError}
		<Alert.Root variant="destructive">
			<Alert.Title>Your saved configuration could not be read</Alert.Title>
			<Alert.Description>
				<p>{configState.loadError.reason}</p>
				{#if configState.loadError.preservedPath}
					<p>
						The original file was kept, unmodified, at
						<code>{configState.loadError.preservedPath}</code>. Fix it by hand and restart the
						app to recover your existing backends and prompts.
					</p>
				{:else}
					<p>Clipboard AI could not preserve the original file either — check the log.</p>
				{/if}
				<p>
					What you see below is a blank starting point, not your saved settings. Any change you
					make here saves immediately — that only creates a fresh config.json and will not touch
					the preserved copy above.
				</p>
			</Alert.Description>
		</Alert.Root>
	{/if}

	{#if configState.error}
		<Alert.Root variant="destructive">
			<Alert.Title>
				{configState.errorSource === 'load' ? 'Couldn’t load config' : 'Couldn’t save config'}
			</Alert.Title>
			<Alert.Description>{configState.error}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if configState.problems.length > 0}
		<Alert.Root>
			<Alert.Title>Saved, but some settings couldn’t be applied</Alert.Title>
			<Alert.Description>
				<ul class="list-disc pl-4">
					{#each configState.problems as problem, i (i)}
						<li>{problem.message}</li>
					{/each}
				</ul>
			</Alert.Description>
		</Alert.Root>
	{/if}

	<Tabs.Root value="backends">
		<Tabs.List>
			<Tabs.Trigger value="backends">Backends</Tabs.Trigger>
			<Tabs.Trigger value="commands">Commands</Tabs.Trigger>
		</Tabs.List>
		<Tabs.Content value="backends">
			{#if configState.loading}
				<p class="text-sm text-muted-foreground">Loading…</p>
			{:else}
				{#key configState.revision}
					<BackendList />
				{/key}
			{/if}
		</Tabs.Content>
		<Tabs.Content value="commands">
			{#if configState.loading}
				<p class="text-sm text-muted-foreground">Loading…</p>
			{:else}
				{#key configState.revision}
					<CommandList />
				{/key}
			{/if}
		</Tabs.Content>
	</Tabs.Root>
</div>
