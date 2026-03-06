export interface ModifierState {
    ctrl: boolean;
    alt: boolean;
    meta: boolean;
}

export function getControlSequence(key: string, modifiers: ModifierState): string | null {
    const k = key.toLowerCase();
    
    if (modifiers.ctrl) {
        if (k >= 'a' && k <= 'z') {
            return String.fromCharCode(k.charCodeAt(0) - 96);
        }
    }
    
    return null;
}

export const SPECIAL_KEYS = {
    ESC: '\x1b',
    TAB: '\t',
    ENTER: '\r',
    UP: '\x1b[A',
    DOWN: '\x1b[B',
    LEFT: '\x1b[D',
    RIGHT: '\x1b[C',
};

// Apply virtual modifier keys to an arrow key escape sequence.
// Arrow base sequences are \x1b[A/B/C/D; modified form is \x1b[1;<n>A/B/C/D
// where n = 1 + bitmask(shift=1, alt=2, ctrl=4).
export function modifiedArrow(base: string, mods: { ctrl: boolean; alt: boolean; meta: boolean }): string {
    let mod = 0;
    if (mods.alt || mods.meta) mod += 2;
    if (mods.ctrl) mod += 4;
    if (mod === 0) return base;
    const letter = base[base.length - 1]; // A, B, C, or D
    return `\x1b[1;${mod + 1}${letter}`;
}