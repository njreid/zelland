import { describe, it, expect } from 'vitest';
import { getControlSequence, SPECIAL_KEYS } from './key-mapper';

describe('key-mapper', () => {
    it('should map ctrl+c to \x03', () => {
        const seq = getControlSequence('c', { ctrl: true, alt: false, meta: false });
        expect(seq).toBe('\x03');
    });

    it('should map ctrl+d to \x04', () => {
        const seq = getControlSequence('d', { ctrl: true, alt: false, meta: false });
        expect(seq).toBe('\x04');
    });

    it('should return null if no modifier is active', () => {
        const seq = getControlSequence('c', { ctrl: false, alt: false, meta: false });
        expect(seq).toBeNull();
    });

    it('should have correct special keys', () => {
        expect(SPECIAL_KEYS.ESC).toBe('\x1b');
        expect(SPECIAL_KEYS.TAB).toBe('\t');
        expect(SPECIAL_KEYS.ENTER).toBe('\r');
    });
});