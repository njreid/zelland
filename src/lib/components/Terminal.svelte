<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Terminal } from '@xterm/xterm';
    import { FitAddon } from '@xterm/addon-fit';
    import '@xterm/xterm/css/xterm.css';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';

    let { tabId } = $props<{ tabId: string }>();
    
    let terminalElement: HTMLDivElement;
    let term: Terminal;
    let fitAddon: FitAddon;
    let unlisten: () => void;

    onMount(async () => {
        term = new Terminal({
            theme: {
                background: '#1a1b26',
                foreground: '#a9b1d6',
            },
            fontFamily: 'monospace',
            cursorBlink: true,
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);
        fitAddon.fit();

        term.onData(async (data) => {
            try {
                const encoder = new TextEncoder();
                const bytes = encoder.encode(data);
                await invoke('mosh_write', { tabId, data: Array.from(bytes) });
            } catch (e) {
                console.error('Failed to write to MOSH:', e);
            }
        });

        unlisten = await listen('mosh-output', (event: any) => {
            if (event.payload.tabId === tabId) {
                term.write(event.payload.data);
            }
        });

        const resizeHandler = () => fitAddon.fit();
        window.addEventListener('resize', resizeHandler);
        
        return () => {
            window.removeEventListener('resize', resizeHandler);
        };
    });

    onDestroy(() => {
        if (term) term.dispose();
        if (unlisten) unlisten();
    });
</script>

<div bind:this={terminalElement} class="terminal-container"></div>

<style>
    .terminal-container {
        width: 100%;
        height: 100%;
        background-color: #1a1b26;
    }
    :global(.xterm) {
        padding: 8px;
    }
</style>
