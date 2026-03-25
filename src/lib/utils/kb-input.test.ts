import { describe, it, expect, vi } from 'vitest';
import { handleKbInput } from './kb-input';

describe('handleKbInput', () => {
    it('encodes ESC sequence to bytes', () => {
        const write = vi.fn();
        handleKbInput('\x1b', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x1b]);
    });

    it('encodes Enter', () => {
        const write = vi.fn();
        handleKbInput('\r', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x0d]);
    });

    it('encodes Tab', () => {
        const write = vi.fn();
        handleKbInput('\t', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x09]);
    });

    it('encodes Ctrl+Up (\\x1b[1;5A)', () => {
        const write = vi.fn();
        handleKbInput('\x1b[1;5A', 'session-1', write);
        expect(write).toHaveBeenCalledWith('session-1', [0x1b, 0x5b, 0x31, 0x3b, 0x35, 0x41]);
    });

    it('does nothing when no active session', () => {
        const write = vi.fn();
        handleKbInput('\r', null, write);
        expect(write).not.toHaveBeenCalled();
    });
});
