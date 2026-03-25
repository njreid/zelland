/**
 * Handles a key sequence event from the native keyboard bar,
 * encoding it to bytes and forwarding to the active session.
 */
export function handleKbInput(
    seq: string,
    activeSessionId: string | null,
    writeInput: (id: string, bytes: number[]) => void,
): void {
    if (!activeSessionId) return;
    writeInput(activeSessionId, Array.from(new TextEncoder().encode(seq)));
}
