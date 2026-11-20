<script module lang="ts">
	export type SectionId = 'commands' | 'backends' | 'history' | 'settings';
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { invoke } from '@tauri-apps/api/core';
	import { activity } from '$lib/state/activity.svelte.js';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import CommandIcon from '@lucide/svelte/icons/command';
	import ServerIcon from '@lucide/svelte/icons/server';
	import HistoryIcon from '@lucide/svelte/icons/rotate-ccw-clock';
	import SettingsIcon from '@lucide/svelte/icons/settings';

	let {
		active,
		onNavigate
	}: {
		active: SectionId;
		onNavigate: (id: SectionId) => void;
	} = $props();

	let version = $state<string | null>(null);

	onMount(() => {
		getVersion().then(
			(v) => (version = v),
			() => (version = null)
		);
	});

	const commandsNeedAttention = $derived(configState.problems.some((p) => p.commandId));

	const items = $derived([
		{ id: 'commands' as const, label: i18n.t.nav.commands, icon: CommandIcon },
		{ id: 'backends' as const, label: i18n.t.nav.backends, icon: ServerIcon },
		{ id: 'history' as const, label: i18n.t.nav.history, icon: HistoryIcon },
		{ id: 'settings' as const, label: i18n.t.nav.settings, icon: SettingsIcon }
	]);
</script>

<nav class="flex h-full w-48 shrink-0 flex-col gap-0.5 px-2.5 py-3" aria-label="Settings sections">
	{#each items as item (item.id)}
		{@const Icon = item.icon}
		{@const isActive = active === item.id}
		<button
			type="button"
			class={[
				'flex h-9 shrink-0 items-center gap-2.5 rounded-[9px] px-2.5 text-left text-[14px] transition-colors',
				isActive
					? 'bg-sidebar-accent text-sidebar-accent-foreground'
					: 'text-sidebar-foreground hover:text-foreground'
			]}
			aria-current={isActive ? 'page' : undefined}
			onclick={() => onNavigate(item.id)}
		>
			<Icon class="size-4 shrink-0" />
			<span class="truncate">{item.label}</span>
			{#if item.id === 'commands' && commandsNeedAttention}
				<span
					class="ml-auto size-[7px] shrink-0 rounded-full bg-destructive"
					title={i18n.t.nav.needsAttention}
				></span>
				<span class="sr-only">{i18n.t.nav.needsAttention}</span>
			{/if}
		</button>
	{/each}

	<div class="mt-auto flex flex-col gap-1 px-2.5 py-2 text-xs text-muted-foreground/70">
		{#if activity.running}
			<div class="flex items-center gap-2 text-muted-foreground" role="status" aria-live="polite">
				<span class="size-2 shrink-0 animate-pulse rounded-full bg-success" aria-hidden="true"></span>
				<span class="truncate">{i18n.t.nav.working}</span>
			</div>
		{/if}
		<button
			type="button"
			class="-mx-1 mb-1 self-start rounded-md px-1 py-0.5 text-[13px] text-muted-foreground transition-colors hover:text-foreground"
			onclick={() => void invoke('quit_app').catch(() => {})}
		>
			{i18n.t.nav.quit}
		</button>
		{#if version}
			<span>{i18n.t.nav.version(version)}</span>
		{/if}
	</div>
</nav>
