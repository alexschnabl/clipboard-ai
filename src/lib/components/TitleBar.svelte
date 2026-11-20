<script lang="ts">
	import { onMount } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';

	let maximized = $state(false);

	function appWindow() {
		try {
			return getCurrentWindow();
		} catch {
			return null;
		}
	}

	onMount(() => {
		const win = appWindow();
		if (!win) return;
		const refresh = () =>
			win.isMaximized().then(
				(m) => (maximized = m),
				() => {}
			);
		void refresh();
		const unlisten = win.onResized(() => void refresh());
		return () => {
			void unlisten.then((f) => f(), () => {});
		};
	});

	const buttons = $derived([
		{ glyph: '', label: 'Minimize', run: () => appWindow()?.minimize(), close: false },
		{
			glyph: maximized ? '' : '',
			label: maximized ? 'Restore' : 'Maximize',
			run: () => appWindow()?.toggleMaximize(),
			close: false
		},
		{ glyph: '', label: 'Close', run: () => appWindow()?.close(), close: true }
	]);
</script>

<div class="flex h-9 shrink-0 items-stretch select-none" data-tauri-drag-region>
	<span
		class="pointer-events-none flex items-center gap-2 pl-4 text-xs text-muted-foreground"
	>
		<img src="/favicon.png" alt="" class="size-3.5 opacity-90" />
		Clipboard AI
	</span>
	<div class="flex-1" data-tauri-drag-region></div>
	{#each buttons as b (b.label)}
		<button
			type="button"
			class={[
				'grid w-[46px] place-items-center text-[10px] text-foreground/80 transition-colors duration-100',
				b.close ? 'hover:bg-[#c42b1c] hover:text-white' : 'hover:bg-foreground/[0.06]'
			]}
			style="font-family: 'Segoe Fluent Icons', 'Segoe MDL2 Assets'"
			aria-label={b.label}
			onclick={() => void b.run()}>{b.glyph}</button
		>
	{/each}
</div>
