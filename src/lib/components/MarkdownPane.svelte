<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { appState } from '$lib/stores/app.svelte';
    import { marked } from 'marked';

    let { filename } = $props<{ filename: string }>();
    let content = $state('');
    let html = $derived(content ? marked.parse(content) : '');

    async function loadContent() {
        // Reset state before loading
        content = '';
        appState.setFileLoaded(filename, false);

        const activeSession = appState.sessions.find(s => s.id === appState.activeSessionId);
        if (!activeSession) return;

        try {
            const host = appState.hosts.find(h => h.address === activeSession.hostAddress);
            const rootPath = host?.projects.find(p => p.session_name === activeSession.zellijSession)?.root_path;
            
            if (rootPath) {
                const daemonUrl = `http://${activeSession.hostAddress}:8083`;
                const path = `${rootPath}/${filename}`;
                
                // The invoke might throw or return an error string depending on Rust implementation
                const res = await invoke<string>("daemon_read_file", { url: daemonUrl, path });
                
                // Heuristic: if it's a "no such file" error string, don't mark as loaded
                if (res.includes("no such file") || res.includes("Status 404")) {
                    console.warn(`File not found on daemon: ${filename}`);
                    appState.setFileLoaded(filename, false);
                } else {
                    content = res;
                    appState.setFileLoaded(filename, true);
                }
            }
        } catch (e) {
            console.error(`Failed to load ${filename}:`, e);
            appState.setFileLoaded(filename, false);
        }
    }

    onMount(() => {
        loadContent();
    });

    $effect(() => {
        if (appState.viewUpdateTrigger && appState.viewUpdateTrigger[filename] !== undefined) {
            loadContent();
        }
    });

    $effect(() => {
        const _sid = appState.activeSessionId;
        loadContent();
    });
</script>

<div class="markdown-pane container" id="pane-{filename}">
    {#if html}
        {@html html}
    {:else}
        <div style="display: flex; justify-content: center; align-items: center; height: 100%; color: var(--fg-dim);">
            <small>No {filename} found for active session.</small>
        </div>
    {/if}
</div>

<style>
    .markdown-pane {
        padding: 1rem;
        height: 100%;
        overflow-y: auto;
    }
</style>
