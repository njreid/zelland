package com.njr.zelland

/**
 * Converts a typed character (from an IME TextWatcher) to its terminal escape sequence,
 * respecting Ctrl and Alt modifier state.
 *
 * This is the companion to [KeybarSeqs.modifiedArrow] which handles arrow keys.
 * Keeping this logic in a standalone object makes it independently testable.
 */
object KeySeqs {

    /**
     * Returns the terminal byte sequence for [ch] with optional Ctrl / Alt modifiers.
     *
     * - Ctrl + letter  →  control character (ANS X3.4: 0x01–0x1A)
     * - Ctrl + `[`     →  ESC (0x1B)
     * - Ctrl + `\`     →  FS  (0x1C)
     * - Ctrl + `]`     →  GS  (0x1D)
     * - Ctrl + `^`     →  RS  (0x1E)
     * - Ctrl + `_`     →  US  (0x1F)
     * - Ctrl + `@`     →  NUL (0x00)
     * - Alt  + any     →  ESC prefix (Meta key emulation)
     * - Newline        →  carriage return (terminals expect CR not LF)
     * - Else           →  character as-is
     */
    fun charToSeq(ch: Char, ctrl: Boolean, alt: Boolean): String = when {
        ctrl && ch.isLetter() -> (ch.lowercaseChar().code and 0x1f).toChar().toString()
        ctrl && ch == '['     -> "\u001b"   // Ctrl+[ = ESC
        ctrl && ch == '\\'    -> "\u001c"   // Ctrl+\ = FS
        ctrl && ch == ']'     -> "\u001d"   // Ctrl+] = GS
        ctrl && ch == '^'     -> "\u001e"   // Ctrl+^ = RS
        ctrl && ch == '_'     -> "\u001f"   // Ctrl+_ = US
        ctrl && ch == '@'     -> "\u0000"   // Ctrl+@ = NUL
        alt                   -> "\u001b$ch"
        ch == '\n'            -> "\r"
        else                  -> ch.toString()
    }
}
