import { describe, it, expect } from 'vitest';
import { getControlSequence, modifiedArrow, SPECIAL_KEYS } from './key-mapper';

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
        expect(SPECIAL_KEYS.UP).toBe('\x1b[A');
        expect(SPECIAL_KEYS.DOWN).toBe('\x1b[B');
        expect(SPECIAL_KEYS.RIGHT).toBe('\x1b[C');
        expect(SPECIAL_KEYS.LEFT).toBe('\x1b[D');
    });
});

describe('modifiedArrow', () => {
    const none = { ctrl: false, alt: false, meta: false };
    const ctrlOnly = { ctrl: true,  alt: false, meta: false };
    const altOnly  = { ctrl: false, alt: true,  meta: false };
    const metaOnly = { ctrl: false, alt: false, meta: true  };
    const ctrlAlt  = { ctrl: true,  alt: true,  meta: false };

    it('returns base sequence unchanged when no modifiers', () => {
        expect(modifiedArrow(SPECIAL_KEYS.UP,    none)).toBe('\x1b[A');
        expect(modifiedArrow(SPECIAL_KEYS.DOWN,  none)).toBe('\x1b[B');
        expect(modifiedArrow(SPECIAL_KEYS.RIGHT, none)).toBe('\x1b[C');
        expect(modifiedArrow(SPECIAL_KEYS.LEFT,  none)).toBe('\x1b[D');
    });

    it('encodes Ctrl+arrow as \\x1b[1;5X', () => {
        expect(modifiedArrow(SPECIAL_KEYS.UP,    ctrlOnly)).toBe('\x1b[1;5A');
        expect(modifiedArrow(SPECIAL_KEYS.DOWN,  ctrlOnly)).toBe('\x1b[1;5B');
        expect(modifiedArrow(SPECIAL_KEYS.RIGHT, ctrlOnly)).toBe('\x1b[1;5C');
        expect(modifiedArrow(SPECIAL_KEYS.LEFT,  ctrlOnly)).toBe('\x1b[1;5D');
    });

    it('encodes Alt+arrow as \\x1b[1;3X', () => {
        expect(modifiedArrow(SPECIAL_KEYS.UP,    altOnly)).toBe('\x1b[1;3A');
        expect(modifiedArrow(SPECIAL_KEYS.DOWN,  altOnly)).toBe('\x1b[1;3B');
        expect(modifiedArrow(SPECIAL_KEYS.RIGHT, altOnly)).toBe('\x1b[1;3C');
        expect(modifiedArrow(SPECIAL_KEYS.LEFT,  altOnly)).toBe('\x1b[1;3D');
    });

    it('treats Meta the same as Alt (\\x1b[1;3X)', () => {
        expect(modifiedArrow(SPECIAL_KEYS.UP,   metaOnly)).toBe('\x1b[1;3A');
        expect(modifiedArrow(SPECIAL_KEYS.DOWN, metaOnly)).toBe('\x1b[1;3B');
    });

    it('encodes Ctrl+Alt+arrow as \\x1b[1;7X', () => {
        expect(modifiedArrow(SPECIAL_KEYS.UP,    ctrlAlt)).toBe('\x1b[1;7A');
        expect(modifiedArrow(SPECIAL_KEYS.DOWN,  ctrlAlt)).toBe('\x1b[1;7B');
        expect(modifiedArrow(SPECIAL_KEYS.RIGHT, ctrlAlt)).toBe('\x1b[1;7C');
        expect(modifiedArrow(SPECIAL_KEYS.LEFT,  ctrlAlt)).toBe('\x1b[1;7D');
    });
});