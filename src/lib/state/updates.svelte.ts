import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UpdateInfo, UpdateProgress } from '$lib/types';

export type UpdatePhase = 'idle' | 'checking' | 'upToDate' | 'available' | 'installing' | 'error';

export class UpdateState {
	phase = $state<UpdatePhase>('idle');
	available = $state<UpdateInfo | null>(null);
	progress = $state<UpdateProgress | null>(null);
	error = $state<string | null>(null);
	installError = $state<string | null>(null);
	promptDismissed = $state(false);
	private started = false;

	async init(): Promise<() => void> {
		if (this.started) return () => {};
		this.started = true;
		const offAvailable = await listen<UpdateInfo>('update://available', (event) => {
			this.offer(event.payload);
		});
		const offProgress = await listen<UpdateProgress>('update://progress', (event) => {
			this.progress = event.payload;
		});
		const pending = await invoke<UpdateInfo | null>('get_available_update').catch(() => null);
		if (pending) this.offer(pending);
		return () => {
			offAvailable();
			offProgress();
			this.started = false;
		};
	}

	private offer(info: UpdateInfo): void {
		if (this.phase === 'installing') return;
		this.available = info;
		this.phase = 'available';
	}

	async check(): Promise<void> {
		if (this.phase === 'checking' || this.phase === 'installing') return;
		this.phase = 'checking';
		this.error = null;
		try {
			const found = await invoke<UpdateInfo | null>('check_for_update');
			if (found) {
				this.available = found;
				this.phase = 'available';
				this.promptDismissed = true;
			} else {
				this.available = null;
				this.phase = 'upToDate';
			}
		} catch (e) {
			this.error = String(e);
			this.phase = 'error';
		}
	}

	async install(): Promise<void> {
		if (!this.available || this.phase === 'installing') return;
		this.phase = 'installing';
		this.progress = null;
		this.installError = null;
		try {
			await invoke('install_update');
		} catch (e) {
			this.installError = String(e);
			this.phase = 'available';
		}
	}

	get percent(): number | null {
		const p = this.progress;
		if (!p || !p.total) return null;
		return Math.min(100, Math.round((p.downloaded / p.total) * 100));
	}
}

export const updateState = new UpdateState();
