<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { appState } from '$lib/stores/app.svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { Channel } from '@tauri-apps/api/core';

    let { tabId } = $props<{ tabId: string }>();

    let terminalElement: HTMLDivElement;
    let resizeObserver: ResizeObserver;
    let destroyed = false;

    function onKbInput(e: Event) {
        const seq: string = (e as CustomEvent).detail?.seq;
        if (!seq) return;
        const bytes = Array.from(new TextEncoder().encode(seq));
        invoke('ssh_write', { tabId, data: bytes }).catch(() => {});
    }

    /** Cell dimensions derived from the font size setting and device DPR. */
    function getCellDimensions() {
        const dpr = window.devicePixelRatio || 1;
        const cellH = appState.terminalFontSize * dpr;
        const cellW = cellH * 0.45;
        return { cellW, cellH, dpr };
    }

    onMount(() => {
        window.addEventListener('kb-input', onKbInput);

        function doFitAndResize() {
            if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                const { cellW, cellH, dpr } = getCellDimensions();
                const cols = Math.floor(terminalElement.clientWidth * dpr / cellW);
                const rows = Math.floor(terminalElement.clientHeight * dpr / cellH);
                if (cols > 0 && rows > 0) {
                    appState.resize(tabId, rows, cols);
                }
            }
        }

        resizeObserver = new ResizeObserver(doFitAndResize);
        resizeObserver.observe(terminalElement);

        const onWindowResize = () => doFitAndResize();
        window.addEventListener('resize', onWindowResize);

        connectSsh();

        return () => {
            window.removeEventListener('kb-input', onKbInput);
            if (resizeObserver) resizeObserver.disconnect();
            window.removeEventListener('resize', onWindowResize);
        };
    });

    type SshMsg =
        | { type: 'Viewport'; data: { data: Uint8Array, at_bottom: boolean, mouse_mode: boolean } }
        | { type: 'Closed' };

    async function connectSsh() {
        const config = appState.getSshConfig(tabId);
        if (!config) return;

        // Initial estimate
        const rows = 24;
        const cols = 80;

        const outputChannel = new Channel<SshMsg>();
        outputChannel.onmessage = (msg) => {
            if (destroyed) return;
            if (msg.type === 'Viewport') {
                // In Phase 3, we don't draw to the DOM.
                // Rust handles native rendering to the SurfaceView.
                // We just keep the channel alive to receive metadata if needed.
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

    // Reactive resize/font-size trigger
    $effect(() => {
        if (appState.terminalResizeTrigger >= 0) {
            setTimeout(() => {
                const { cellW, cellH, dpr } = getCellDimensions();
                // Update Rust renderer font size first so cell layout matches.
                invoke('set_terminal_font_size', {
                    cssPx: appState.terminalFontSize,
                    dpr
                }).catch(() => {});
                if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                    const cols = Math.floor(terminalElement.clientWidth * dpr / cellW);
                    const rows = Math.floor(terminalElement.clientHeight * dpr / cellH);
                    if (cols > 0 && rows > 0) {
                        appState.resize(tabId, rows, cols);
                    }
                }
            }, 200);
        }
    });

    onDestroy(() => {
        destroyed = true;
        invoke('ssh_disconnect', { tabId }).catch(() => {});
    });
</script>

<!-- The container is now transparent, revealing the SurfaceView underneath -->
<div bind:this={terminalElement} class="terminal-container"></div>

<style>
    .terminal-container {
        width: 100%;
        height: 100%;
        background-color: transparent; /* Reveal native surface */
        overflow: hidden;
    }
</style>
