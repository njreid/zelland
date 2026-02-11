<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Terminal } from '@xterm/xterm';
    import { FitAddon } from '@xterm/addon-fit';
    import '@xterm/xterm/css/xterm.css';
    import { appState } from '$lib/stores/app.svelte';
    import { listen } from '@tauri-apps/api/event';

    let { tabId } = $props<{ tabId: string }>();
    
    let terminalElement: HTMLDivElement;
    let term: Terminal;
    let fitAddon: FitAddon;
    let unlisteners: (() => void)[] = [];
    let resizeObserver: ResizeObserver;

    onMount(() => {
        term = new Terminal({
            theme: {
                background: '#1a1b26',
                foreground: '#a9b1d6',
            },
            fontFamily: 'InconsolataGoNerdFontMono, Monaco, monospace',
            fontSize: appState.terminalFontSize,
            cursorBlink: true,
            scrollback: 0,
            allowProposedApi: true
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);
        
        // Use ResizeObserver for robust fitting
        resizeObserver = new ResizeObserver(() => {
            if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                fitAddon.fit();
                const dims = fitAddon.proposeDimensions();
                if (dims) {
                    appState.resize(tabId, dims.rows, dims.cols);
                }
            }
        });
        resizeObserver.observe(terminalElement);

        term.onData(async (data) => {
            const encoder = new TextEncoder();
            const bytes = encoder.encode(data);
            await appState.writeInput(tabId, Array.from(bytes));
        });

        // Listen to both output types
        listen('ssh-output', (event: any) => {
            if (event.payload.tabId === tabId) {
                term.write(event.payload.data);
            }
        }).then(u => unlisteners.push(u));

        return () => {
            if (resizeObserver) resizeObserver.disconnect();
        };
    });

    // Reactive focus trigger
    $effect(() => {
        if (appState.terminalFocusTrigger > 0 && term) {
            term.focus();
        }
    });

    // Reactive resize trigger
    $effect(() => {
        if (appState.terminalResizeTrigger >= 0 && term && fitAddon) {
            // Update font size if it changed
            if (term.options.fontSize !== appState.terminalFontSize) {
                term.options.fontSize = appState.terminalFontSize;
            }
            
            // Wait a bit for MOSH/PTY to stabilize
            setTimeout(() => {
                if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                    fitAddon.fit();
                    const dims = fitAddon.proposeDimensions();
                    if (dims) {
                        appState.resize(tabId, dims.rows, dims.cols);
                    }
                }
            }, 200);
        }
    });

    onDestroy(() => {
        if (term) term.dispose();
        unlisteners.forEach(u => u());
    });

    export function focus() {
        if (term) term.focus();
    }
</script>

<div bind:this={terminalElement} class="terminal-container"></div>

<style>
    .terminal-container {
        width: 100%;
        height: 100%;
        background-color: var(--pico-background-color);
        overflow: hidden;
    }
    :global(.xterm) {
        padding: 0.25rem;
    }
    :global(.xterm-viewport) {
        overflow-y: auto !important;
    }
</style>