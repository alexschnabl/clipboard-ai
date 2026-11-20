<script lang="ts">
	import { Button } from '$lib/components/shadcn/button/index.js';
	import * as Alert from '$lib/components/shadcn/alert/index.js';
	import * as Dialog from '$lib/components/shadcn/dialog/index.js';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import ActivityIcon from '@lucide/svelte/icons/activity';
	import { fade } from 'svelte/transition';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import TerminalIcon from '@lucide/svelte/icons/square-terminal';
	import CpuIcon from '@lucide/svelte/icons/cpu';
	import WebhookIcon from '@lucide/svelte/icons/webhook';
	import BackendForm from './BackendForm.svelte';
	import PageHeader from './PageHeader.svelte';
	import StatusLine from './StatusLine.svelte';
	import { configState } from '$lib/state/config.svelte.js';
	import { healthState } from '$lib/state/health.svelte.js';
	import { healthDetail, healthTone, resultSentence } from '$lib/health-format';
	import { i18n } from '$lib/i18n/i18n.svelte';
	import { KIND_LABELS, type BackendConfig, type BackendKind } from '$lib/types';

	const KIND_ICONS: Record<BackendKind, typeof SparklesIcon> = {
		'anthropic-api': SparklesIcon,
		'claude-cli': TerminalIcon,
		'openai-compatible': CpuIcon,
		http: WebhookIcon
	};

	let formOpen = $state(false);
	let editingBackend = $state<BackendConfig | null>(null);
	let blockedDelete = $state<{ backendName: string; usedBy: string[] } | null>(null);
	let pendingDelete = $state<BackendConfig | null>(null);
	let confirmDeleteOpen = $state(false);

	function openAdd(): void {
		editingBackend = null;
		blockedDelete = null;
		formOpen = true;
	}

	function openEdit(backend: BackendConfig): void {
		editingBackend = backend;
		blockedDelete = null;
		formOpen = true;
	}

	function handleDelete(backend: BackendConfig): void {
		pendingDelete = backend;
		confirmDeleteOpen = true;
	}

	async function confirmDelete(): Promise<void> {
		if (!pendingDelete) return;
		const target = pendingDelete;
		confirmDeleteOpen = false;
		pendingDelete = null;
		formOpen = false;
		const result = await configState.deleteBackend(target.id);
		if (result.ok) healthState.forget(target.id);
		blockedDelete =
			result.usedBy.length > 0 ? { backendName: target.name, usedBy: result.usedBy } : null;
	}

	function cancelDelete(): void {
		confirmDeleteOpen = false;
		pendingDelete = null;
	}

	function usedBy(backend: BackendConfig): string[] {
		return configState.commandsUsingBackend(backend.id).map((c) => c.name);
	}

	function detail(backend: BackendConfig): string {
		return [backend.model, backend.baseUrl].filter((v) => v && v.length > 0).join(' - ');
	}

	const ids = $derived(configState.config.backends.map((b) => b.id));
	const anyChecking = $derived(ids.some((id) => healthState.isChecking(id)));
</script>

{#if !formOpen}
	<div class="flex h-full min-h-0 flex-col" in:fade={{ duration: 120 }}>
		<PageHeader
			title={i18n.t.backends.title}
			subtitle={i18n.t.backends.count(configState.config.backends.length)}
		>
			{#snippet actions()}
				{#if ids.length > 0}
					<Button
						variant="outline"
						size="sm"
						disabled={anyChecking}
						onclick={() => void healthState.checkAll(ids)}
					>
						<ActivityIcon class="size-4" />
						{i18n.t.backends.checkAll}
					</Button>
				{/if}
				<Button size="sm" onclick={openAdd}>
					<PlusIcon class="size-4" />
					{i18n.t.backends.addBackend}
				</Button>
			{/snippet}
		</PageHeader>

		<div class="page-body">
			{#if blockedDelete}
				<Alert.Root variant="destructive">
					<Alert.Title>{i18n.t.backends.blockedDeleteTitle(blockedDelete.backendName)}</Alert.Title>
					<Alert.Description>
						{i18n.t.backends.blockedDeleteDescription(blockedDelete.usedBy.join(', '))}
					</Alert.Description>
				</Alert.Root>
			{/if}

			<section>
				<div class="group-title">
					<span>{i18n.t.backends.groupTitle}</span>
					<span class="text-xs text-muted-foreground/80">{i18n.t.backends.groupHint}</span>
				</div>
				<div class="list-card">
					{#each configState.config.backends as backend (backend.id)}
						{@const users = usedBy(backend)}
						{@const KindIcon = KIND_ICONS[backend.kind]}
						{@const result = healthState.results[backend.id]}
						{@const checking = healthState.isChecking(backend.id)}
						<div class="list-row relative">
							<button
								type="button"
								class="row-link absolute inset-0"
								onclick={() => openEdit(backend)}
								aria-label={i18n.t.backends.editAria(backend.name)}
							></button>
							<span class="pointer-events-none grid size-9 shrink-0 place-items-center rounded-[9px] border bg-canvas text-muted-foreground">
								<KindIcon class="size-4" />
							</span>
							<div class="pointer-events-none min-w-0 flex-1">
								<div class="flex items-baseline gap-2">
									<span class="truncate" title={backend.name}>{backend.name}</span>
									<span class="shrink-0 text-xs text-muted-foreground">{KIND_LABELS[backend.kind]}</span>
								</div>
								{#if result || checking}
									<StatusLine
										tone={healthTone(result, checking)}
										text={resultSentence(result, checking, i18n.t.health, i18n.locale)}
										title={healthDetail(result)}
										class="pointer-events-auto mt-1 w-fit max-w-full text-[13px] text-muted-foreground"
									/>
								{:else if detail(backend)}
									<div class="mt-1 truncate text-[13px] text-muted-foreground">{detail(backend)}</div>
								{/if}
							</div>
							<span class="pointer-events-none max-w-48 shrink-0 truncate text-right text-xs text-muted-foreground/80">
								{users.length > 0 ? i18n.t.backends.usedBy(users.join(', ')) : i18n.t.backends.unused}
							</span>
							<Button
								variant="ghost"
								size="sm"
								class="relative h-7 shrink-0 px-2.5 text-[12.5px] text-muted-foreground"
								disabled={checking}
								onclick={() => void healthState.check(backend.id)}
								aria-label={i18n.t.backends.checkAria(backend.name)}
							>
								{i18n.t.backends.check}
							</Button>
							<ChevronRightIcon class="pointer-events-none size-4 shrink-0 text-muted-foreground/60" />
						</div>
					{:else}
						<div class="px-5 py-8 text-center text-sm text-muted-foreground">
							{i18n.t.backends.empty}
						</div>
					{/each}
				</div>
			</section>
		</div>
	</div>
{/if}

<BackendForm
	bind:open={formOpen}
	backend={editingBackend}
	onDelete={editingBackend ? () => editingBackend && handleDelete(editingBackend) : undefined}
/>

<Dialog.Root bind:open={confirmDeleteOpen}>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>{i18n.t.backends.deleteTitle(pendingDelete?.name ?? '')}</Dialog.Title>
			<Dialog.Description>
				{i18n.t.backends.deleteDescription}
			</Dialog.Description>
		</Dialog.Header>
		<Dialog.Footer>
			<Button variant="outline" onclick={cancelDelete}>{i18n.t.common.cancel}</Button>
			<Button variant="destructive" onclick={() => void confirmDelete()}
				>{i18n.t.common.delete}</Button
			>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
