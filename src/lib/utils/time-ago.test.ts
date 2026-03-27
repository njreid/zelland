import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { timeAgo } from './time-ago';

describe('timeAgo', () => {
    const NOW = new Date('2026-01-01T12:00:00Z').getTime();

    beforeEach(() => {
        vi.useFakeTimers();
        vi.setSystemTime(NOW);
    });

    afterEach(() => {
        vi.useRealTimers();
    });

    it('returns "just now" for 0 seconds ago', () => {
        expect(timeAgo(new Date(NOW).toISOString())).toBe('just now');
    });

    it('returns "just now" for 59 seconds ago', () => {
        expect(timeAgo(new Date(NOW - 59_000).toISOString())).toBe('just now');
    });

    it('returns "1m ago" at exactly 60 seconds', () => {
        expect(timeAgo(new Date(NOW - 60_000).toISOString())).toBe('1m ago');
    });

    it('returns minutes for < 60 minutes', () => {
        expect(timeAgo(new Date(NOW - 5 * 60_000).toISOString())).toBe('5m ago');
        expect(timeAgo(new Date(NOW - 59 * 60_000).toISOString())).toBe('59m ago');
    });

    it('returns "1h ago" at exactly 60 minutes', () => {
        expect(timeAgo(new Date(NOW - 3_600_000).toISOString())).toBe('1h ago');
    });

    it('returns hours for < 24 hours', () => {
        expect(timeAgo(new Date(NOW - 3 * 3_600_000).toISOString())).toBe('3h ago');
        expect(timeAgo(new Date(NOW - 23 * 3_600_000).toISOString())).toBe('23h ago');
    });

    it('returns "1d ago" at exactly 24 hours', () => {
        expect(timeAgo(new Date(NOW - 86_400_000).toISOString())).toBe('1d ago');
    });

    it('returns days for >= 24 hours', () => {
        expect(timeAgo(new Date(NOW - 2 * 86_400_000).toISOString())).toBe('2d ago');
        expect(timeAgo(new Date(NOW - 30 * 86_400_000).toISOString())).toBe('30d ago');
    });
});
