// test/unit/time-format.test.ts — formatting rules for HistoryPanel's Time
// and Duration columns. Both formatters take their "current time" (and, for
// the timestamp one, locale) as explicit parameters precisely so these rules
// are testable without mocking the system clock or depending on whatever
// locale happens to run the suite.
import { describe, expect, it } from 'vitest';
import { formatDuration, formatHistoryTimestamp } from '../../src/lib/time-format';

describe('formatHistoryTimestamp', () => {
	// Built with the local-time `Date` constructor, not `Date.UTC` --
	// `formatHistoryTimestamp` compares `getFullYear`/`getMonth`/`getDate`
	// (local-time accessors) and `Intl.DateTimeFormat` also renders in local
	// time by default, so a UTC-anchored fixture would silently land on a
	// different calendar day than intended on any machine not itself in UTC
	// (exactly what broke this test the first time it ran here).
	// 2026-08-08 14:32:07 local, as the fixed "now" for every "today" case.
	const today = new Date(2026, 7, 8, 14, 32, 7).getTime();

	it('shows a bare time for a timestamp earlier the same day', () => {
		const earlierToday = new Date(2026, 7, 8, 9, 5, 0).getTime();
		expect(formatHistoryTimestamp(earlierToday, today, 'en-US')).toBe('09:05 AM');
	});

	it('shows a bare time for a timestamp later the same day', () => {
		const laterToday = new Date(2026, 7, 8, 23, 59, 0).getTime();
		expect(formatHistoryTimestamp(laterToday, today, 'en-US')).toBe('11:59 PM');
	});

	it('shows a date, not a time, for yesterday', () => {
		const yesterday = new Date(2026, 7, 7, 23, 59, 0).getTime();
		expect(formatHistoryTimestamp(yesterday, today, 'en-US')).toBe('Aug 7');
	});

	it('includes the year once the entry is from a different calendar year', () => {
		const lastYear = new Date(2025, 11, 25, 10, 0, 0).getTime();
		expect(formatHistoryTimestamp(lastYear, today, 'en-US')).toBe('Dec 25, 2025');
	});

	it('omits the year for an older entry still within the current year', () => {
		const earlierThisYear = new Date(2026, 0, 3, 10, 0, 0).getTime();
		expect(formatHistoryTimestamp(earlierThisYear, today, 'en-US')).toBe('Jan 3');
	});

	it('is locale-aware: the same instant renders differently for a different locale', () => {
		const earlierToday = new Date(2026, 7, 8, 9, 5, 0).getTime();
		const en = formatHistoryTimestamp(earlierToday, today, 'en-US');
		const de = formatHistoryTimestamp(earlierToday, today, 'de-DE');
		expect(en).not.toBe(de);
	});
});

describe('formatDuration', () => {
	it('renders a null or undefined duration as an em dash', () => {
		expect(formatDuration(null)).toBe('—');
		expect(formatDuration(undefined)).toBe('—');
	});

	it('renders sub-second durations in whole milliseconds, not a rounded second', () => {
		expect(formatDuration(600)).toBe('600 ms');
		expect(formatDuration(0)).toBe('0 ms');
	});

	it('renders one-second-and-over durations in seconds to one decimal place', () => {
		expect(formatDuration(1800)).toBe('1.8 s');
		expect(formatDuration(45_000)).toBe('45.0 s');
		expect(formatDuration(1000)).toBe('1.0 s');
	});
});
