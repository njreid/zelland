<script lang="ts">
    import { onMount } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { projectStore } from '$lib/stores/project.svelte';
    import { marked } from 'marked';

    let { filename } = $props<{ filename: string }>();
    let content = $state('Loading...');
    let html = $derived(marked.parse(content));

    onMount(async () => {
        if (projectStore.currentProject) {
            try {
                const daemonUrl = `http://${projectStore.currentProject.host}:8080`;
                const path = `${projectStore.currentProject.rootPath}/${filename}`;
                content = await invoke<string>("daemon_read_file", { url: daemonUrl, path });
            } catch (e) {
                content = `Error loading ${filename}: ${e}`;
            }
        } else {
            content = `No project active to load ${filename}`;
        }
    });
</script>

<div class="markdown-pane p-4 overflow-y-auto h-full prose prose-invert max-w-none">
    {@html html}
</div>

<style>
    .markdown-pane {
        background-color: var(--bg-main);
        color: var(--fg-main);
    }
</style>
