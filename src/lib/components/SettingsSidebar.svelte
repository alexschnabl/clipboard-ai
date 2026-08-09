<!--
	lib/components/SettingsSidebar.svelte — vertical nav rail for the settings window.

	Shell for the chosen design direction (.superpowers/mockups/mockup-a-compact.html,
	"Mockup A: Compact Utility"): a ~176px rail of icon + label + count badge replaces
	all top chrome, including the page title -- the OS title bar already names the app.

	The mockup pinned a System/Light/Dark theme toggle to the bottom of this rail; theme
	was later moved to the Settings section (one place it's set), freeing that footer. It
	now holds the in-window activity indicator: while a hotkey cycle runs, a pulsing dot
	appears here, mirroring the tray-icon breathing off the same `cycle://state` event --
	so a slow backend is visible from inside the window too, not only in the tray.
-->
<script module lang="ts">
	export type SectionId = 'commands' | 'backends' | 'history' | 'settings';
</script>

<script lang="ts">
	import { cn } from '$lib/utils.js';
	import { activity } from '$lib/state/activity.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import RiCommandLine from 'remixicon-svelte/icons/command-line';
	import RiServerLine from 'remixicon-svelte/icons/server-line';
	import RiHistoryLine from 'remixicon-svelte/icons/history-line';
	import RiSettingsLine from 'remixicon-svelte/icons/settings-line';

	let {
		active,
		onNavigate,
		commandCount,
		backendCount
	}: {
		active: SectionId;
		onNavigate: (id: SectionId) => void;
		commandCount: number;
		backendCount: number;
	} = $props();

	const items = $derived([
		{ id: 'commands' as const, label: i18n.t.nav.commands, icon: RiCommandLine, count: commandCount },
		{ id: 'backends' as const, label: i18n.t.nav.backends, icon: RiServerLine, count: backendCount },
		{ id: 'history' as const, label: i18n.t.nav.history, icon: RiHistoryLine, count: undefined },
		{ id: 'settings' as const, label: i18n.t.nav.settings, icon: RiSettingsLine, count: undefined }
	]);
</script>

<nav
	class="flex h-full w-44 shrink-0 flex-col gap-0.5 border-r border-sidebar-border bg-sidebar p-2"
	aria-label="Settings sections"
>
	{#each items as item (item.id)}
		{@const Icon = item.icon}
		{@const isActive = active === item.id}
		<button
			type="button"
			class={cn(
				'flex h-[30px] shrink-0 items-center gap-2 rounded-sm border border-transparent px-2 text-left text-[12.5px] font-medium text-sidebar-foreground hover:bg-sidebar-accent',
				isActive && 'border-sidebar-border bg-sidebar-accent font-semibold text-sidebar-accent-foreground'
			)}
			aria-current={isActive ? 'page' : undefined}
			onclick={() => onNavigate(item.id)}
		>
			<Icon class="size-[15px] shrink-0 opacity-80" />
			<span class="truncate">{item.label}</span>
			{#if item.count !== undefined}
				<!-- F3 (review3-frontend.md): text-muted-foreground on bg-muted is
				     4.15:1 in light mode, under AA's 4.5:1 -- text-foreground/65
				     clears both themes (6.03:1 light, 6.88:1 dark), the same fix
				     TABLE_HEAD_CLASS in $lib/table.ts carries. -->
				<span
					class="ml-auto flex h-[15px] shrink-0 items-center rounded-full bg-muted px-1.5 text-[10.5px] font-semibold text-foreground/65"
				>
					{item.count}
				</span>
			{/if}
		</button>
	{/each}

	<!-- Pinned to the bottom (`mt-auto`). Only present while a cycle is
	     visibly running; `role="status"` + `aria-live` so a screen reader
	     announces it, since it conveys transient state with no other cue. -->
	{#if activity.running}
		<div
			class="mt-auto flex items-center gap-2 px-2 py-1.5 text-[11px] text-muted-foreground"
			role="status"
			aria-live="polite"
		>
			<span class="size-2 shrink-0 animate-pulse rounded-full bg-primary" aria-hidden="true"></span>
			<span class="truncate">{i18n.t.nav.working}</span>
		</div>
	{/if}
</nav>
