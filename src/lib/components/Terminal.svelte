<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Terminal } from '@xterm/xterm';
    import { FitAddon } from '@xterm/addon-fit';
    import '@xterm/xterm/css/xterm.css';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';

    let { tabId, data } = $props();
    
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

        term.onData((data) => {
            // Send to Rust
            // TODO: Implementation for interactive sessions
        });

        // Listen for output from Rust
        unlisten = await listen('ssh-output', (event: any) => {
            if (event.payload.tabId === tabId) {
                term.write(event.payload.data);
            }
        });

        window.addEventListener('resize', () => fitAddon.fit());
    });

    onDestroy(() => {
        if (term) term.dispose();
        if (unlisten) unlisten();
    });
</script>

<div bind:this={terminalElement} class="w-full h-full bg-[#1a1b26]"></div>

<style>
    :global(.xterm) {
        padding: 8px;
    }
</style>
