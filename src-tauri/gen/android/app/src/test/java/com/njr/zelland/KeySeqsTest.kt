package com.njr.zelland

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Unit tests for [KeySeqs.charToSeq] — the IME character → terminal escape sequence mapping.
 *
 * These tests verify the exact byte sequences emitted for Ctrl/Alt modifier combinations,
 * which are critical for correct terminal behaviour (Ctrl+C, Ctrl+Z, Alt+arrows, etc.).
 */
class KeySeqsTest {

    // -----------------------------------------------------------------------
    // Plain characters (no modifiers)
    // -----------------------------------------------------------------------

    @Test fun plainLowercaseLetter() {
        assertEquals("a", KeySeqs.charToSeq('a', ctrl = false, alt = false))
    }

    @Test fun plainUppercaseLetter() {
        assertEquals("Z", KeySeqs.charToSeq('Z', ctrl = false, alt = false))
    }

    @Test fun plainDigit() {
        assertEquals("5", KeySeqs.charToSeq('5', ctrl = false, alt = false))
    }

    @Test fun plainSpace() {
        assertEquals(" ", KeySeqs.charToSeq(' ', ctrl = false, alt = false))
    }

    @Test fun newlineToCarriageReturn() {
        // IME sends LF; terminal expects CR.
        assertEquals("\r", KeySeqs.charToSeq('\n', ctrl = false, alt = false))
    }

    // -----------------------------------------------------------------------
    // Ctrl + letter  →  control character (0x01–0x1A)
    // -----------------------------------------------------------------------

    @Test fun ctrlA() = assertEquals("\u0001", KeySeqs.charToSeq('a', ctrl = true, alt = false))
    @Test fun ctrlB() = assertEquals("\u0002", KeySeqs.charToSeq('b', ctrl = true, alt = false))
    @Test fun ctrlC() = assertEquals("\u0003", KeySeqs.charToSeq('c', ctrl = true, alt = false))
    @Test fun ctrlD() = assertEquals("\u0004", KeySeqs.charToSeq('d', ctrl = true, alt = false))
    @Test fun ctrlH() = assertEquals("\u0008", KeySeqs.charToSeq('h', ctrl = true, alt = false))
    @Test fun ctrlL() = assertEquals("\u000c", KeySeqs.charToSeq('l', ctrl = true, alt = false))
    @Test fun ctrlU() = assertEquals("\u0015", KeySeqs.charToSeq('u', ctrl = true, alt = false))
    @Test fun ctrlW() = assertEquals("\u0017", KeySeqs.charToSeq('w', ctrl = true, alt = false))
    @Test fun ctrlZ() = assertEquals("\u001a", KeySeqs.charToSeq('z', ctrl = true, alt = false))

    @Test fun ctrlUpperCaseTreatedSameAsLower() {
        // Both produce the same control character regardless of case.
        assertEquals(
            KeySeqs.charToSeq('c', ctrl = true, alt = false),
            KeySeqs.charToSeq('C', ctrl = true, alt = false)
        )
    }

    // -----------------------------------------------------------------------
    // Ctrl + punctuation
    // -----------------------------------------------------------------------

    @Test fun ctrlOpenBracketIsEsc()   = assertEquals("\u001b", KeySeqs.charToSeq('[',  ctrl = true, alt = false))
    @Test fun ctrlBackslashIsFs()      = assertEquals("\u001c", KeySeqs.charToSeq('\\', ctrl = true, alt = false))
    @Test fun ctrlCloseBracketIsGs()   = assertEquals("\u001d", KeySeqs.charToSeq(']',  ctrl = true, alt = false))
    @Test fun ctrlCaretIsRs()          = assertEquals("\u001e", KeySeqs.charToSeq('^',  ctrl = true, alt = false))
    @Test fun ctrlUnderscoreIsUs()     = assertEquals("\u001f", KeySeqs.charToSeq('_',  ctrl = true, alt = false))
    @Test fun ctrlAtIsNul()            = assertEquals("\u0000", KeySeqs.charToSeq('@',  ctrl = true, alt = false))

    // -----------------------------------------------------------------------
    // Alt + character  →  ESC prefix (Meta emulation)
    // -----------------------------------------------------------------------

    @Test fun altLowercaseLetter() = assertEquals("\u001ba", KeySeqs.charToSeq('a', ctrl = false, alt = true))
    @Test fun altUppercaseLetter() = assertEquals("\u001bB", KeySeqs.charToSeq('B', ctrl = false, alt = true))
    @Test fun altDot()             = assertEquals("\u001b.", KeySeqs.charToSeq('.', ctrl = false, alt = true))
    @Test fun altDigit()           = assertEquals("\u001b1", KeySeqs.charToSeq('1', ctrl = false, alt = true))

    // Alt + newline: newline path does not apply when alt is set (alt branch wins first).
    @Test fun altNewline() = assertEquals("\u001b\n", KeySeqs.charToSeq('\n', ctrl = false, alt = true))

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    @Test fun ctrlDigitPassesThrough() {
        // Ctrl+digit has no defined control code — falls through to digit as-is.
        // (No ctrl+letter or ctrl+punct match; alt=false; char != '\n' → else branch.)
        assertEquals("1", KeySeqs.charToSeq('1', ctrl = true, alt = false))
    }
}
