// lib/state/activity.svelte.ts — whether a hotkey cycle is currently running,
// for the in-window indicator in the nav rail.
//
// Fed by the `cycle://state` event the Rust side emits from `tray_anim` (the
// same signal that drives the tray-icon breathing), wired up once in
// `+page.svelte`. A singleton rune so `SettingsSidebar` can read it directly
// without threading a prop down through the shell. `running` only goes true
// once a cycle has run past the delay gate -- a fast cycle never flips it, by
// design (see `tray_anim`'s emit).

class Activity {
	running = $state(false);
}

export const activity = new Activity();
