import { invoke } from '@tauri-apps/api/core';
import { iconToDataUrl, normalizeExe } from '$lib/editor';
import type { RunningApp } from '$lib/types';

export class RunningApps {
	list = $state<RunningApp[]>([]);
	icons = $state<Record<string, string>>({});
	loading = $state(false);
	loaded = $state(false);
	failed = $state(false);
	private pending: Promise<void> | null = null;

	refresh(): Promise<void> {
		if (this.pending) return this.pending;
		this.loading = true;
		this.failed = false;
		const run = async (): Promise<void> => {
			try {
				const apps = (await invoke<RunningApp[] | null>('list_running_apps')) ?? [];
				const icons = { ...this.icons };
				for (const app of apps) {
					const url = iconToDataUrl(app.icon);
					if (url) icons[normalizeExe(app.exe)] = url;
				}
				this.icons = icons;
				this.list = apps;
			} catch {
				this.failed = true;
				this.list = [];
			} finally {
				this.loading = false;
				this.loaded = true;
				this.pending = null;
			}
		};
		this.pending = run();
		return this.pending;
	}

	ensure(): void {
		if (!this.loaded && !this.loading) void this.refresh();
	}

	iconFor(exe: string): string | undefined {
		return this.icons[normalizeExe(exe)];
	}
}

export const runningApps = new RunningApps();
