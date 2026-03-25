<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { appState } from '$lib/stores/app.svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { Channel } from '@tauri-apps/api/core';

    let { tabId } = $props<{ tabId: string }>();

    let terminalElement: HTMLDivElement;
    let resizeObserver: ResizeObserver;
    let destroyed = false;

    onMount(() => {
        function doFitAndResize() {
            if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                // Standardized grid dimensions matching Rust backend (CELL_WIDTH=24.0, CELL_HEIGHT=32.0)
                const cols = Math.floor(terminalElement.clientWidth / 24.0);
                const rows = Math.floor(terminalElement.clientHeight / 32.0);
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

    // Reactive resize trigger
    $effect(() => {
        if (appState.terminalResizeTrigger >= 0) {
            setTimeout(() => {
                if (terminalElement.clientWidth > 0 && terminalElement.clientHeight > 0) {
                    const cols = Math.floor(terminalElement.clientWidth / 24.0);
                    const rows = Math.floor(terminalElement.clientHeight / 32.0);
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
