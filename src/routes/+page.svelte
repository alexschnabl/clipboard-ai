<!-- routes/+page.svelte — settings window: Backends and Commands tabs -->
<script lang="ts">
	import * as Tabs from '$lib/components/shadcn/tabs/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Badge } from '$lib/components/shadcn/badge/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import BackendList from '$lib/components/BackendList.svelte';
	import CommandList from '$lib/components/CommandList.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';

	let confirmingClose = $state(false);

	onMount(() => {
		void configState.load();
	});

	// Mirrors `configState.dirty` into Rust on every change, in both
	// directions -- Rust cannot see Svelte state, and the settings window's
	// close handler cannot ask across the IPC bridge without risking a
	// deadlock (see lib.rs), so the frontend pushes instead. Firing on the
	// clean transition too, not just the dirty one, is what keeps a
	// successful Save from leaving Rust thinking there is still something
	// unsaved and nagging on the very next close.
	$effect(() => {
		void invoke('set_dirty', { dirty: configState.dirty });
	});

	// Rust asks for confirmation here only when there is something unsaved
	// (see `handle_close_request` in commands.rs) and only waits briefly for
	// this listener to answer before hiding the window regardless -- so
	// `ack_close_prompt` is called immediately, the instant the dialog is on
	// screen, not once the user has actually decided. That decision itself
	// (Cancel / Discard / Save) is unbounded: the window simply stays open
	// until one of the three handlers below runs.
	onMount(() => {
		const unlisten = listen('settings://close-requested', () => {
			confirmingClose = true;
			void invoke('ack_close_prompt');
		});
		return () => {
			void unlisten.then((f) => f());
		};
	});

	// The settings window is hidden, never destroyed (see `hide_and_reapply`
	// in commands.rs), so anything transient a running Svelte tree holds --
	// a `HotkeyRecorder` mid-recording, an open `BackendForm`/`CommandForm`
	// dialog with a stale draft -- would otherwise survive straight through
	// a hide/reshow cycle. Rust emits this event every time it actually
	// hides the window (every route: the immediate-hide path, the
	// close-confirmation timeout fallback, and `force_close_settings`);
	// bumping `configState.revision` here, consumed by the `{#key}` blocks
	// below, remounts the tab bodies so all of that resets. Listening for
	// this event specifically, rather than `document.visibilitychange`, is
	// deliberate -- WebView2 firing that on a native window hide is not
	// guaranteed.
	onMount(() => {
		const unlisten = listen('settings://window-hidden', () => {
			configState.revision++;
		});
		return () => {
			void unlisten.then((f) => f());
		};
	});

	async function discardAndClose(): Promise<void> {
		confirmingClose = false;
		await configState.load(); // drop local edits
		await invoke('force_close_settings');
	}

	async function saveAndClose(): Promise<void> {
		if (await configState.save()) {
			confirmingClose = false;
			await invoke('force_close_settings');
		}
		// else: save() already populated configState.error / .problems: leave
		// the dialog open (and the window open) so the user can see why.
	}
</script>

<div class="flex flex-col gap-6 p-8">
	<div class="flex items-center justify-between gap-4">
		<div>
			<h1 class="text-2xl font-semibold">Clipboard AI</h1>
			<p class="text-sm text-muted-foreground">Configure backends and commands.</p>
		</div>
		<div class="flex items-center gap-3">
			{#if configState.dirty}
				<Badge variant="outline">Unsaved changes</Badge>
			{/if}
			<Button
				onclick={() => void configState.save()}
				disabled={configState.saving || configState.loading}
			>
				{configState.saving ? 'Saving…' : 'Save'}
			</Button>
		</div>
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
					What you see below is a blank starting point, not your saved settings. You can still
					save it — that only creates a fresh config.json and will not touch the preserved copy
					above.
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

<Dialog.Root bind:open={confirmingClose}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Unsaved changes</Dialog.Title>
			<Dialog.Description>
				You have unsaved changes to backends or commands. Save them before closing, or discard
				them?
			</Dialog.Description>
		</Dialog.Header>

		{#if configState.error}
			<Alert.Root variant="destructive">
				<Alert.Title>
					{configState.errorSource === 'load' ? 'Couldn’t load config' : 'Couldn’t save config'}
				</Alert.Title>
				<Alert.Description>{configState.error}</Alert.Description>
			</Alert.Root>
		{/if}

		<Dialog.Footer>
			<Button variant="outline" onclick={() => (confirmingClose = false)}>Cancel</Button>
			<Button variant="outline" onclick={() => void discardAndClose()}>Discard</Button>
			<Button onclick={() => void saveAndClose()} disabled={configState.saving}>
				{configState.saving ? 'Saving…' : 'Save and close'}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
