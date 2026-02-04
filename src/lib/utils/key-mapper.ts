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