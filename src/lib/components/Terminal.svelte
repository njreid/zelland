<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { Terminal } from '@xterm/xterm';
    import { FitAddon } from '@xterm/addon-fit';
    import { WebglAddon } from '@xterm/addon-webgl';
    import { CanvasAddon } from '@xterm/addon-canvas';
    import '@xterm/xterm/css/xterm.css';
    import { appState } from '$lib/stores/app.svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { Channel } from '@tauri-apps/api/core';

    let { tabId } = $props<{ tabId: string }>();

    let terminalElement: HTMLDivElement;
    let term: Terminal;
    let fitAddon: FitAddon;
    let resizeObserver: ResizeObserver;
    let destroyed = false;
    let isScrolling = false;
    let mouseReportingEnabled = $state(false);

    onMount(() => {
        term = new Terminal({
            theme: {
                background: '#1a1b26',
                foreground: '#a9b1d6',
            },
            fontFamily: 'InconsolataGoNerdFontMono, Monaco, monospace',
            fontSize: appState.terminalFontSize,
            fontWeight: appState.terminalFontWeight as any,
            cursorBlink: true,
            scrollback: 0,
            allowProposedApi: true
        });

        fitAddon = new FitAddon();
        term.loadAddon(fitAddon);
        term.open(terminalElement);

        // Prefer WebGL renderer; fall back to Canvas if WebGL context is unavailable
        // (Canvas is still ~2x faster than the default DOM renderer)
        try {
            const webgl = new WebglAddon();
            webgl.onContextLoss(() => {
                webgl.dispose();
                term.loadAddon(new CanvasAddon());
            });
            term.loadAddon(webgl);
        } catch {
            term.loadAddon(new CanvasAddon());
        }

        function doFitAndResize() {
            if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                fitAddon.fit();
                const dims = fitAddon.proposeDimensions();
                if (dims) {
                    appState.resize(tabId, dims.rows, dims.cols);
                }
            }
        }

        // Use ResizeObserver for robust fitting
        resizeObserver = new ResizeObserver(doFitAndResize);
        resizeObserver.observe(terminalElement);

        // Backup: window resize event catches cases ResizeObserver misses
        // (e.g. vertical-only resizes in some WebKit builds)
        const onWindowResize = () => doFitAndResize();
        window.addEventListener('resize', onWindowResize);

        // Intercept wheel events: if the terminal hasn't requested mouse reporting (like in Zellij's
        // normal mode), we pipe the wheel directly to Zellij's scroll-up/down actions.
        let lastScrollTime = 0;
        terminalElement.addEventListener('wheel', (e) => {
            if (mouseReportingEnabled) return; // Let xterm.js handle it if Zellij asked for mouse

            e.preventDefault();
            const now = Date.now();
            if (now - lastScrollTime < 30) return; // Throttle to ~30 FPS for smooth feel without spam
            lastScrollTime = now;

            const action = e.deltaY < 0 ? 'scroll-up' : 'scroll-down';
            appState.runZellijAction(tabId, action);
        }, { passive: false });

        // Ensure clicking focuses the terminal
        terminalElement.addEventListener('mousedown', () => {
            if (term) term.focus();
        });

        term.onData((data) => {
            // Any key press exits scrollback and snaps back to live view
            if (isScrolling) {
                isScrolling = false;
                appState.scrollTerminal(tabId, 0); // 0 = snap to live
            }
            const bytes = new TextEncoder().encode(data);
            appState.writeInput(tabId, bytes);
        });

        // Kick off the SSH connection in the background — doesn't block mount.
        connectSsh();

        return () => {
            if (resizeObserver) resizeObserver.disconnect();
            window.removeEventListener('resize', onWindowResize);
        };
    });

    type SshMsg =
        | { type: 'Viewport'; data: { data: Uint8Array, at_bottom: bool, mouse_mode: bool } }
        | { type: 'Closed' };

    async function connectSsh() {
        const config = appState.getSshConfig(tabId);
        if (!config) return;

        // Propose initial dimensions if not already set
        let rows = 24;
        let cols = 80;
        if (fitAddon) {
            const dims = fitAddon.proposeDimensions();
            if (dims) {
                rows = dims.rows;
                cols = dims.cols;
            }
        }

        const outputChannel = new Channel<SshMsg>();
        outputChannel.onmessage = (msg) => {
            if (destroyed) return;
            if (msg.type === 'Viewport') {
                // Strategy 1: Replace terminal content with Rust-rendered grid snapshot.
                term.clear();
                term.write(msg.data.data);
                
                // Update frontend state tracking
                mouseReportingEnabled = msg.data.mouse_mode;

                if (msg.data.at_bottom) {
                    isScrolling = false;
                }
            } else if (msg.type === 'Closed') {
                appState.onSessionClosed(tabId);
            }
        };

        try {
            await invoke('ssh_connect', { tabId, config, rows, cols, channel: outputChannel });
            if (!destroyed) appState.onSessionConnected(tabId);
        } catch (e) {
            if (!destroyed) appState.onSessionConnectionFailed(tabId, String(e));
        }
    }

    // Reactive focus trigger
    $effect(() => {
        if (appState.terminalFocusTrigger > 0 && term) {
            term.focus();
        }
    });

    // Reactive resize trigger
    $effect(() => {
        if (appState.terminalResizeTrigger >= 0 && term && fitAddon) {
            console.log(`Terminal: resize effect triggered (trigger=${appState.terminalResizeTrigger})`);
            // Update font options if they changed
            if (term.options.fontSize !== appState.terminalFontSize) {
                console.log(`Terminal: updating fontSize from ${term.options.fontSize} to ${appState.terminalFontSize}`);
                term.options.fontSize = appState.terminalFontSize;
            }
            if (String(term.options.fontWeight) !== String(appState.terminalFontWeight)) {
                console.log(`Terminal: updating fontWeight from ${term.options.fontWeight} to ${appState.terminalFontWeight}`);
                term.options.fontWeight = appState.terminalFontWeight as any;
                term.refresh(0, term.rows - 1);
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
        destroyed = true;
        if (term) term.dispose();
        invoke('ssh_disconnect', { tabId }).catch(() => {});
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
