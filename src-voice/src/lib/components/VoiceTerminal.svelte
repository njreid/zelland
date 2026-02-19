<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Terminal } from '@xterm/xterm';
    import { FitAddon } from '@xterm/addon-fit';
    import '@xterm/xterm/css/xterm.css';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';

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
                cursor: '#c0caf5',
                selectionBackground: '#33467c',
            },
            fontFamily: 'Inconsolata, Consolas, Monaco, monospace',
            fontSize: 14,
            cursorBlink: true,
            scrollback: 5000,
            allowProposedApi: true,
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);

        function doFitAndResize() {
            if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                fitAddon.fit();
                const dims = fitAddon.proposeDimensions();
                if (dims) {
                    invoke('pty_resize', { rows: dims.rows, cols: dims.cols }).catch(() => {});
                }
            }
        }

        resizeObserver = new ResizeObserver(doFitAndResize);
        resizeObserver.observe(terminalElement);

        const onWindowResize = () => doFitAndResize();
        window.addEventListener('resize', onWindowResize);

        // Terminal input → PTY
        term.onData((data) => {
            const encoder = new TextEncoder();
            const bytes = Array.from(encoder.encode(data));
            invoke('pty_write', { data: bytes });
        });

        // PTY output → Terminal
        listen<{ data: string }>('pty-output', (event) => {
            term.write(event.payload.data);
        }).then(u => unlisteners.push(u));

        // PTY closed
        listen('pty-closed', () => {
            term.writeln('\r\n[Process exited]');
        }).then(u => unlisteners.push(u));

        // Spawn PTY shell
        invoke('pty_spawn').then(() => {
            setTimeout(doFitAndResize, 100);
        }).catch((e) => {
            term.writeln(`Failed to spawn shell: ${e}`);
        });

        return () => {
            resizeObserver.disconnect();
            window.removeEventListener('resize', onWindowResize);
        };
    });

    onDestroy(() => {
        if (term) term.dispose();
        unlisteners.forEach(u => u());
    });

    export function sendToPty(text: string) {
        const encoder = new TextEncoder();
        const bytes = Array.from(encoder.encode(text));
        invoke('pty_write', { data: bytes });
    }
</script>

<div bind:this={terminalElement} class="terminal-container"></div>

<style>
    .terminal-container {
        width: 100%;
        height: 100%;
        background-color: var(--bg);
        overflow: hidden;
    }
    :global(.xterm) {
        padding: 4px;
    }
    :global(.xterm-viewport) {
        overflow-y: auto !important;
    }
</style>
