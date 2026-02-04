<script lang="ts">
    import { ChevronUp, ChevronDown, ChevronLeft, ChevronRight, CornerDownLeft, ArrowRightToLine } from 'lucide-svelte';
    import { sessionStore } from '$lib/stores/session.svelte';
    import { SPECIAL_KEYS } from '$lib/utils/key-mapper';
    import { invoke } from '@tauri-apps/api/core';

    let ctrl = $state(false);
    let alt = $state(false);
    let meta = $state(false);
    let showAlphaGrid = $state(false);

    function sendKey(seq: string) {
        console.log('Sending key sequence:', seq);
        // TODO: Send to active terminal via Rust
        if (ctrl || alt || meta) {
            ctrl = alt = meta = false;
            showAlphaGrid = false;
        }
    }

    function toggleModifier(mod: 'ctrl' | 'alt' | 'meta') {
        if (mod === 'ctrl') ctrl = !ctrl;
        if (mod === 'alt') alt = !alt;
        if (mod === 'meta') meta = !meta;
        
        showAlphaGrid = ctrl || alt || meta;
    }

    const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'.split('');
</script>

<div class="flex flex-col bg-[#1e1e2e] border-t border-[#313244] pb-safe">
    {#if showAlphaGrid}
        <div class="grid grid-cols-9 gap-1 p-2 bg-[#181825]">
            {#each alphabet as char}
                <button 
                    class="h-10 rounded bg-[#313244] text-[#cdd6f4] active:bg-[#45475a]"
                    onclick={() => sendKey(char)}
                >
                    {char}
                </button>
            {each}
        </div>
    {/if}

    <div class="flex items-center justify-between h-14 px-2 gap-2">
        <div class="flex gap-1">
            <button 
                class="w-12 h-10 rounded font-bold transition-colors {ctrl ? 'bg-[#fab387] text-[#11111b]' : 'bg-[#313244] text-[#cdd6f4]'}"
                onclick={() => toggleModifier('ctrl')}
            >
                C
            </button>
            <button 
                class="w-12 h-10 rounded font-bold transition-colors {alt ? 'bg-[#f9e2af] text-[#11111b]' : 'bg-[#313244] text-[#cdd6f4]'}"
                onclick={() => toggleModifier('alt')}
            >
                A
            </button>
            <button 
                class="w-12 h-10 rounded font-bold transition-colors {meta ? 'bg-[#a6e3a1] text-[#11111b]' : 'bg-[#313244] text-[#cdd6f4]'}"
                onclick={() => toggleModifier('meta')}
            >
                M
            </button>
        </div>

        <div class="flex gap-1 items-center">
            <button class="p-2 rounded bg-[#313244] text-[#cdd6f4]" onclick={() => sendKey(SPECIAL_KEYS.ESC)}>ESC</button>
            <button class="p-2 rounded bg-[#313244] text-[#cdd6f4]" onclick={() => sendKey(SPECIAL_KEYS.TAB)}><ArrowRightToLine size={20} /></button>
            
            <div class="grid grid-cols-3 gap-0.5">
                <div />
                <button class="p-1 rounded bg-[#313244]" onclick={() => sendKey(SPECIAL_KEYS.UP)}><ChevronUp size={16} /></button>
                <div />
                <button class="p-1 rounded bg-[#313244]" onclick={() => sendKey(SPECIAL_KEYS.LEFT)}><ChevronLeft size={16} /></button>
                <button class="p-1 rounded bg-[#313244]" onclick={() => sendKey(SPECIAL_KEYS.DOWN)}><ChevronDown size={16} /></button>
                <button class="p-1 rounded bg-[#313244]" onclick={() => sendKey(SPECIAL_KEYS.RIGHT)}><ChevronRight size={16} /></button>
            </div>

            <button class="p-2 rounded bg-[#89b4fa] text-[#11111b]" onclick={() => sendKey(SPECIAL_KEYS.ENTER)}><CornerDownLeft size={20} /></button>
        </div>
    </div>
</div>
