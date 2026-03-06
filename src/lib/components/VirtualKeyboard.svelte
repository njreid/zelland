<script lang="ts">
    import { CornerDownLeft, ArrowRightToLine, Menu, ChevronLeft, ChevronUp, ChevronDown, ChevronRight, Move } from 'lucide-svelte';
    import { appState } from '$lib/stores/app.svelte';
    import { SPECIAL_KEYS, modifiedArrow } from '$lib/utils/key-mapper';
    import { onMount } from 'svelte';

    let { onToggleSidebar } = $props();

    let ctrl = $state(false);
    let alt  = $state(false);
    let meta = $state(false);
    let arrowsOpen = $state(false);

    function resetModifiers() {
        ctrl = false;
        alt  = false;
        meta = false;
    }

    // Send a literal sequence, applying any active modifiers if it's an arrow key.
    function sendKey(seq: string) {
        if (appState.activeSessionId) {
            appState.writeInput(appState.activeSessionId, Array.from(new TextEncoder().encode(seq)));
        }
        resetModifiers();
    }

    // Arrow keys need modifier encoding (\x1b[1;5A for Ctrl+Up, etc.)
    function sendArrow(base: string) {
        sendKey(modifiedArrow(base, { ctrl, alt, meta }));
    }

    async function sendZellijTab(n: number) {
        if (appState.activeSessionId) {
            await appState.runZellijAction(appState.activeSessionId, `go-to-tab ${n}`);
        }
    }

    function toggleModifier(mod: 'ctrl' | 'alt' | 'meta') {
        if (mod === 'ctrl') ctrl = !ctrl;
        if (mod === 'alt')  alt  = !alt;
        if (mod === 'meta') meta = !meta;
        appState.triggerTerminalFocus();
    }

    // Apply virtual modifiers to physical keyboard input while any modifier is active.
    // Also handle arrow keys from a physical keyboard.
    onMount(() => {
        window.addEventListener('keydown', (e) => {
            if (!ctrl && !alt && !meta) return;

            // Arrow keys with modifiers
            const arrowMap: Record<string, string> = {
                ArrowUp: SPECIAL_KEYS.UP, ArrowDown: SPECIAL_KEYS.DOWN,
                ArrowLeft: SPECIAL_KEYS.LEFT, ArrowRight: SPECIAL_KEYS.RIGHT,
            };
            if (arrowMap[e.key]) {
                e.preventDefault();
                e.stopPropagation();
                sendArrow(arrowMap[e.key]);
                return;
            }

            if (e.key.length === 1 || e.key === 'Enter' || e.key === 'Tab') {
                e.preventDefault();
                e.stopPropagation();

                let char = e.key;
                if (e.key === 'Enter') char = '\r';
                if (e.key === 'Tab')   char = '\t';

                if (alt) char = `\x1b${char}`;
                if (ctrl && char.length === 1) {
                    const code = char.toUpperCase().charCodeAt(0);
                    if (code >= 64 && code <= 95) char = String.fromCharCode(code - 64);
                }
                if (meta && !alt) char = `\x1b${char}`;

                sendKey(char);
            }
        }, true);
    });
</script>

<div class="keyboard-root pb-safe">
    <!-- Arrow row: shown when arrowsOpen -->
    {#if arrowsOpen}
        <div class="arrow-row">
            <button class="outline contrast icon-btn key-unit" onclick={() => sendArrow(SPECIAL_KEYS.LEFT)}><ChevronLeft  size={16} /></button>
            <button class="outline contrast icon-btn key-unit" onclick={() => sendArrow(SPECIAL_KEYS.UP)}><ChevronUp    size={16} /></button>
            <button class="outline contrast icon-btn key-unit" onclick={() => sendArrow(SPECIAL_KEYS.DOWN)}><ChevronDown  size={16} /></button>
            <button class="outline contrast icon-btn key-unit" onclick={() => sendArrow(SPECIAL_KEYS.RIGHT)}><ChevronRight size={16} /></button>
        </div>
    {/if}

    <!-- Main mod-bar -->
    <div class="mod-bar">
        <!-- Left: Menu + modifiers -->
        <div class="group">
            <button class="outline contrast icon-btn key-unit" onclick={() => onToggleSidebar()} title="Menu">
                <Menu size={18} />
            </button>
            <div role="group" class="mb-0">
                <button class="{ctrl ? 'primary' : 'outline contrast'} key-unit" onclick={() => toggleModifier('ctrl')}>C</button>
                <button class="{alt  ? 'primary' : 'outline contrast'} key-unit" onclick={() => toggleModifier('alt')}>A</button>
                <button class="{meta ? 'primary' : 'outline contrast'} key-unit" onclick={() => toggleModifier('meta')}>M</button>
            </div>
        </div>

        <!-- Center: tab buttons (hidden on narrow screens) -->
        <div class="group flex-grow justify-center hide-on-narrow">
            <div role="group" class="mb-0">
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(1)}>1</button>
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(2)}>2</button>
                <button class="outline contrast key-unit tab-btn" onclick={() => sendZellijTab(3)}>3</button>
            </div>
        </div>

        <!-- Right: ESC, Tab, arrows toggle, Enter -->
        <div class="group">
            <button class="outline contrast key-unit"     onclick={() => sendKey(SPECIAL_KEYS.ESC)}>ESC</button>
            <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.TAB)}><ArrowRightToLine size={18} /></button>
            <button
                class="{arrowsOpen ? 'primary' : 'outline contrast'} icon-btn key-unit"
                onclick={() => arrowsOpen = !arrowsOpen}
                title="Arrow keys"
            >
                <Move size={18} />
            </button>
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

    /* Arrow row: right-aligned, same padding as mod-bar */
    .arrow-row {
        display: flex;
        justify-content: flex-end;
        align-items: center;
        gap: 0.35rem;
        padding: 0.25rem 0.5rem 0;
    }

    .group {
        display: flex;
        gap: 0.5rem;
        align-items: center;
    }

    .flex-grow   { flex-grow: 1; }
    .justify-center { justify-content: center; }

    @media (max-width: 600px) {
        .hide-on-narrow { display: none; }
    }

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

    .tab-btn {
        background-color: #2a2b3d;
        border-color: #4a4b5d;
        color: #c0caf5;
    }
    .tab-btn:active { background-color: var(--pico-primary); color: white; }

    .icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
    }

    [role="group"] { margin-bottom: 0 !important; }
</style>
