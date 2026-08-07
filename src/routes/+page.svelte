<!-- routes/+page.svelte — settings window: Backends and Commands tabs -->
<script lang="ts">
	import * as Tabs from '$lib/components/shadcn/tabs/index.js';
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import { Badge } from '$lib/components/shadcn/badge/index.js';
	import BackendList from '$lib/components/BackendList.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { onMount } from 'svelte';

	onMount(() => {
		void configState.load();
	});
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

	{#if configState.error}
		<Alert.Root variant="destructive">
			<Alert.Title>Couldn’t save config</Alert.Title>
			<Alert.Description>{configState.error}</Alert.Description>
		</Alert.Root>
	{/if}

	{#if configState.problems.length > 0}
		<Alert.Root>
			<Alert.Title>Saved, but some settings couldn’t be applied</Alert.Title>
			<Alert.Description>
				<ul class="list-disc pl-4">
					{#each configState.problems as problem, i (i)}
						<li>{problem}</li>
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
				<BackendList />
			{/if}
		</Tabs.Content>
		<Tabs.Content value="commands">
			<p class="text-sm text-muted-foreground">Commands — coming soon.</p>
		</Tabs.Content>
	</Tabs.Root>
</div>
