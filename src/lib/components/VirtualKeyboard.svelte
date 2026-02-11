<script lang="ts">
    import { CornerDownLeft, ArrowRightToLine, Menu } from 'lucide-svelte';
    import { appState } from '$lib/stores/app.svelte';
    import { SPECIAL_KEYS } from '$lib/utils/key-mapper';
    import { onMount } from 'svelte';
    import ArrowPad from './ArrowPad.svelte';

    let { onToggleSidebar } = $props();

    let ctrl = $state(false);
    let alt = $state(false);
    let meta = $state(false);

    // Send a specific sequence directly (for click buttons)
    function sendKey(seq: string) {
        if (appState.activeSessionId) {
            appState.writeInput(appState.activeSessionId, Array.from(new TextEncoder().encode(seq)));
        }
        // Reset modifiers after click action? Usually yes for single-shot modifiers
        resetModifiers();
    }

    // Send Zellij tab switch command
    async function sendZellijTab(n: number) {
        if (appState.activeSessionId) {
            await appState.runZellijAction(appState.activeSessionId, `go-to-tab ${n}`);
        }
    }

    function toggleModifier(mod: 'ctrl' | 'alt' | 'meta') {
        if (mod === 'ctrl') ctrl = !ctrl;
        if (mod === 'alt') alt = !alt;
        if (mod === 'meta') meta = !meta;
        
        // Return focus to terminal after picking a modifier
        appState.triggerTerminalFocus();
    }

    function resetModifiers() {
        ctrl = false;
        alt = false;
        meta = false;
    }

    // Listen for physical keyboard events to apply virtual modifiers
    function handleGlobalKeydown(e: KeyboardEvent) {
        if (!ctrl && !alt && !meta) return;

        // Prevent default if we are modifying it
        // e.preventDefault(); // Be careful with this, might block normal typing too aggressively

        // Logic to construct modified key sequence
        // This is complex. For now, let's assume we just want to send the modified key code.
        // A simpler approach for "add modifier to next key" is tricky with web key events.
        // We might need to rely on the terminal handling standard keyboard events
        // and only intercept if we are "injecting" modifiers.
        
        // Actually, xterm.js handles keyboard input. 
        // If we want these toggle buttons to affect the physical keyboard, 
        // we need to intercept the key at the window level, prevent xterm from seeing the original,
        // and send a custom sequence.
        
        // Simplified approach: These modifiers only affect the NEXT click on the virtual bar 
        // OR we try to simulate it.
        // But the prompt says "add their modifier to the next key entered by the system keyboard".
        
        // Let's defer strict system keyboard interception for a moment and focus on layout 
        // as the "system keyboard" on mobile usually doesn't show up unless an input is focused.
        // If xterm textarea is focused, it receives input.
        
        // Implementation:
        // We capture 'keydown' on window during capture phase.
        // If modifiers are active, we stop propagation, construct the sequence, send it, and reset modifiers.
    }

    onMount(() => {
       window.addEventListener('keydown', (e) => {
           if (ctrl || alt || meta) {
               // Only intercept single character keys or known functional keys
               if (e.key.length === 1 || e.key === 'Enter' || e.key === 'Tab') {
                   e.preventDefault();
                   e.stopPropagation();
                   
                   // Construct simplified sequence (basic support)
                   // Real SSH modifiers are hard. 
                   // Ctrl+char -> charCode - 64 (for A-Z)
                   // Alt+char -> ESC + char
                   
                   let char = e.key;
                   if (e.key === 'Enter') char = '\r';
                   if (e.key === 'Tab') char = '\t';

                   // Alt handler
                   if (alt) {
                       char = `\x1b${char}`;
                   }
                   
                   // Ctrl handler (basic)
                   if (ctrl && char.length === 1) {
                       const code = char.toUpperCase().charCodeAt(0);
                       if (code >= 64 && code <= 95) {
                           char = String.fromCharCode(code - 64);
                       }
                   }

                   // Meta/Super handler (often same as Alt in terminals or ignored)
                   // We'll treat as Alt for now
                   if (meta && !alt) {
                        char = `\x1b${char}`;
                   }

                   sendKey(char);
               }
           }
       }, true);
    });
</script>

<div class="keyboard-root pb-safe">
    <div class="mod-bar">
        <!-- Left Group: Menu + Modifiers -->
        <div class="group">
            <button class="outline contrast icon-btn key-unit" onclick={() => onToggleSidebar()} title="Menu">
                <Menu size={18} />
            </button>
            
            <div role="group" class="mb-0">
                <button 
                    class="{ctrl ? 'primary' : 'outline contrast'} key-unit"
                    onclick={() => toggleModifier('ctrl')}
                >
                    C
                </button>
                <button 
                    class="{alt ? 'primary' : 'outline contrast'} key-unit"
                    onclick={() => toggleModifier('alt')}
                >
                    A
                </button>
                <button 
                    class="{meta ? 'primary' : 'outline contrast'} key-unit"
                    onclick={() => toggleModifier('meta')}
                >
                    M
                </button>
            </div>
        </div>

        <!-- Center Group: Dynamic Tabs (Hidden on small screens if needed, or flexible) -->
        <div class="group flex-grow justify-center hide-on-narrow">
            <div role="group" class="mb-0">
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(1)}>1</button>
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(2)}>2</button>
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(3)}>3</button>
            </div>
        </div>

        <!-- Right Group: Nav + Arrows + Enter -->
        <div class="group">
            <button class="outline contrast key-unit" onclick={() => sendKey(SPECIAL_KEYS.ESC)}>ESC</button>
            <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.TAB)}><ArrowRightToLine size={18} /></button>
            
            <ArrowPad sendKey={sendKey} />

            <button class="primary icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.ENTER)}><CornerDownLeft size={18} /></button>
        </div>
    </div>
</div>

<style>
    .keyboard-root {
        display: flex;
        flex-direction: column;
        background-color: var(--bg-keyboard);
        border-top: 1px solid var(--pico-border-color);
        position: relative;
        z-index: 100;
    }

    .mod-bar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.25rem 0.5rem;
        gap: 0.5rem;
        overflow-x: auto;
        scrollbar-width: none;
    }
    .mod-bar::-webkit-scrollbar { display: none; }

    .group {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .flex-grow {
        flex-grow: 1;
    }
    
    .justify-center {
        justify-content: center;
    }

    /* Hide tab buttons if screen is too narrow (approx < 600px) 
       Adjust breakpoint as needed for "if horizontal space" requirement */
    @media (max-width: 600px) {
        .hide-on-narrow {
            display: none;
        }
    }

    /* Fixed unit size for all buttons */
    .key-unit {
        width: 2.2rem;
        height: 2.2rem;
        min-width: 2.2rem;
        padding: 0 !important;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.75rem;
        margin-bottom: 0 !important;
        line-height: 1;
        border-width: 1px;
    }

    /* Tab buttons specific styling */
    .tab-btn {
        background-color: #2a2b3d; /* Different color */
        border-color: #4a4b5d;
        color: #c0caf5;
    }
    .tab-btn:active {
        background-color: var(--pico-primary);
        color: white;
    }

    .icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
    }

    /* Override Pico's group margin */
    [role="group"] {
        margin-bottom: 0 !important;
    }
</style>