import { invoke } from '@tauri-apps/api/core';
import type { Health } from '$lib/types';

export interface HealthResult {
	health: Health | null;
	error: string | null;
}

const GOOD: Health['status'][] = ['ok', 'cold', 'slow', 'reachable', 'cliReady', 'cliUnknownAuth'];

export function isWorking(result: HealthResult | undefined): boolean {
	return !!result?.health && GOOD.includes(result.health.status);
}

export class HealthState {
	results = $state<Record<string, HealthResult>>({});
	pending = $state<Record<string, boolean>>({});
	private inFlight = new Map<string, Promise<void>>();

	check(id: string): Promise<void> {
		const running = this.inFlight.get(id);
		if (running) return running;
		this.pending[id] = true;
		const task = (async () => {
			try {
				const health = await invoke<Health>('check_backend', { backendId: id });
				this.results[id] = { health, error: null };
			} catch (e) {
				this.results[id] = { health: null, error: String(e) };
			} finally {
				delete this.pending[id];
				this.inFlight.delete(id);
			}
		})();
		this.inFlight.set(id, task);
		return task;
	}

	async checkAll(ids: string[]): Promise<void> {
		await Promise.all(ids.map((id) => this.check(id)));
	}

	forget(id: string): void {
		delete this.results[id];
	}

	isChecking(id: string): boolean {
		return this.pending[id] === true;
	}

	anyWorking(ids: string[]): boolean {
		return ids.some((id) => isWorking(this.results[id]));
	}

	allCheckedAndFailing(ids: string[]): boolean {
		return (
			ids.length > 0 && ids.every((id) => this.results[id] && !isWorking(this.results[id]))
		);
	}
}

export const healthState = new HealthState();
