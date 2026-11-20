import { invoke } from '@tauri-apps/api/core';
import type { HistoryEntry } from '$lib/types';

export class OnboardingState {
	sticky = $state(false);
	succeeded = $state(false);
	historyKnown = $state(false);
	hiddenLocally = $state(false);

	async refreshHistory(): Promise<void> {
		try {
			const history = await invoke<HistoryEntry[]>('get_history');
			this.succeeded = history.some((e) => e.outcome.status === 'succeeded');
		} catch {
			this.succeeded = false;
		}
		this.historyKnown = true;
	}

	shouldShow(input: {
		dismissed: boolean;
		isDefaultSetup: boolean;
		allBackendsFailing: boolean;
	}): boolean {
		if (input.dismissed || this.hiddenLocally || !this.historyKnown) return false;
		if (this.sticky) return true;
		return !this.succeeded && (input.isDefaultSetup || input.allBackendsFailing);
	}
}

export const onboarding = new OnboardingState();
