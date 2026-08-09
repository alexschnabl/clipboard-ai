<!-- routes/+page.svelte — settings window shell: sidebar navigation over Commands,
     Backends, History and Settings. No page title here by design -- see
     SettingsSidebar.svelte and .superpowers/mockups/mockup-a-compact.html; the OS
     title bar already names the app, so a second "Clipboard AI" header was dropped. -->
<script lang="ts">
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import SettingsSidebar, { type SectionId } from '$lib/components/SettingsSidebar.svelte';
	import BackendList from '$lib/components/BackendList.svelte';
	import CommandList from '$lib/components/CommandList.svelte';
	import HistoryPanel from '$lib/components/HistoryPanel.svelte';
	import SettingsPanel from '$lib/components/SettingsPanel.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { onMount } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	// Which section is showing. Not reset on the revision bump below -- that
	// remounts *content*, but bits-ui's old Tabs.Root managed the active tab
	// outside the remounted subtree too, so a hide/reshow previously left the
	// user on whatever tab they'd been on. Same behaviour here: this lives
	// outside the `{#key}` block.
	let activeSection = $state<SectionId>('commands');

	onMount(() => {
		void configState.load();

		// The settings window is hidden, never destroyed (see
		// `hide_and_reapply` in commands.rs), so anything transient a running
		// Svelte tree holds -- a `HotkeyRecorder` mid-recording, an open
		// `BackendForm`/`CommandForm` dialog with a stale draft -- would
		// otherwise survive straight through a hide/reshow cycle. Rust emits
		// this event every time it actually hides the window; bumping
		// `configState.revision` here, consumed by the `{#key}` block below,
		// remounts the active section's content so all of that resets.
		// Listening for this event specifically, rather than
		// `document.visibilitychange`, is deliberate -- WebView2 firing that
		// on a native window hide is not guaranteed.
		const unlisten = listen('settings://window-hidden', () => {
			configState.revision++;
		});
		return () => {
			void unlisten.then((f) => f());
		};
	});
</script>

<!-- `h-screen`, not `h-full`: the root layout's `<main>` (+layout.svelte)
     intentionally keeps `min-h-screen`, not a definite height, so this
     reaches for the viewport unit directly instead of
     depending on an ancestor whose sizing isn't this component's to change.
     `overflow-hidden` here, plus `overflow-hidden` on the section wrapper below
     and `overflow-y-auto` scoped to just the alerts strip and each section's own
     content-scroll region, is what keeps this a fixed-chrome shell with internal
     scrolling, matching the mockup's `.window`/`.content-scroll` split, rather
     than a page that scrolls as a whole. -->
<div class="flex h-screen min-h-0 overflow-hidden">
	<SettingsSidebar
		active={activeSection}
		onNavigate={(id) => (activeSection = id)}
		commandCount={configState.config.commands.length}
		backendCount={configState.config.backends.length}
	/>

	<div class="flex min-w-0 flex-1 flex-col overflow-hidden">
		{#if configState.loadError || configState.error || configState.problems.length > 0}
			<!-- Page-level alerts, kept out of the scrolling content region below so
			     they stay visible while a long list scrolls underneath. Capped so a
			     pile of problems can't push the whole section out of view at the
			     720x520 minimum window size. `px-4` matches the 16px gutter every
			     section's own toolbar uses below, so this banner lines up with the
			     content it sits above rather than looking like a different scale. -->
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
			<!-- Not `overflow-y-auto` here anymore: the previous single page-level
			     scroll region made every section's own "Add command"/"Add backend"
			     toolbar scroll away with the list under it, and put a sticky
			     column-header's stick point wherever the *page* happened to be
			     scrolled to rather than the section's own list. Each section below
			     now owns a `content-toolbar` (pinned) + `content-scroll` (scrolls)
			     split matching the mockup's `.content` structure directly, so this
			     wrapper only needs to constrain height and let the active section
			     manage its own internal scrolling. -->
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
