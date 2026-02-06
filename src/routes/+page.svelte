<script lang="ts">
    import { onMount } from 'svelte';
    import { projectStore } from '$lib/stores/project.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import MarkdownPane from '$lib/components/MarkdownPane.svelte';
    import VirtualKeyboard from '$lib/components/VirtualKeyboard.svelte';
    import { invoke } from '@tauri-apps/api/core';

    let activePane = $state(0);
    let daemonUrl = $state('http://10.0.0.1:8080'); // Example default
    let isLoading = $state(false);

    onMount(async () => {
        // Initial fetch of projects if daemon is reachable
        await projectStore.fetchProjects(daemonUrl);
    });

    async function handleProjectSelect(projectId: string) {
        isLoading = true;
        try {
            await projectStore.activateProject(daemonUrl, projectId);
            // After activation, start mosh
            if (projectStore.currentProject) {
                await invoke("mosh_connect", {
                    tabId: "main",
                    config: {
                        host: projectStore.currentProject.host,
                        port: 22,
                        username: "njr", // TODO: Make configurable
                        auth_method: "Password",
                        password: "password", // TODO: Make configurable
                        session_name: projectStore.currentProject.session_name
                    }
                });
            }
        } catch (e) {
            console.error("Failed to activate project:", e);
        } finally {
            isLoading = false;
        }
    }
</script>

<div class="app-root h-screen flex flex-col overflow-hidden">
    <div class="ribbon-container flex-1 overflow-x-auto overflow-y-hidden snap-x snap-mandatory flex">
        <!-- Pane 0: Selection or Terminal -->
        <section class="pane snap-start min-w-full h-full relative">
            {#if projectStore.currentProject}
                <Terminal tabId="main" />
            {:else}
                <div class="welcome-screen flex flex-col items-center justify-center p-8 h-full">
                    <h1 class="text-2xl font-bold mb-4 text-accent">Zelland</h1>
                    <div class="project-list w-full max-w-md bg-darker p-4 rounded-lg border border-border">
                        <h2 class="text-sm font-bold uppercase mb-4 text-fg-dim">Select Project</h2>
                        {#each projectStore.projects as project}
                            <button 
                                class="project-row w-full text-left p-3 rounded mb-2 hover:bg-border transition-colors"
                                onclick={() => handleProjectSelect(project.id)}
                            >
                                <div class="font-bold">{project.name || project.id}</div>
                                <div class="text-xs text-fg-dim">{project.host} - {project.root_path}</div>
                            </button>
                        {/each}
                        <button class="btn-primary w-full py-2 mt-4 font-bold" onclick={() => projectStore.fetchProjects(daemonUrl)}>
                            Refresh List
                        </button>
                    </div>
                </div>
            {/if}
        </section>

        <!-- Pane 1: README.md -->
        <section class="pane snap-start min-w-full h-full border-l border-border">
            <MarkdownPane filename="README.md" />
        </section>

        <!-- Pane 2: PLAN.md -->
        <section class="pane snap-start min-w-full h-full border-l border-border">
            <MarkdownPane filename="PLAN.md" />
        </section>

        <!-- Pane 3: DESIGN.md -->
        <section class="pane snap-start min-w-full h-full border-l border-border">
            <MarkdownPane filename="DESIGN.md" />
        </section>
    </div>

    <!-- Bottom Controls -->
    <VirtualKeyboard onToggleSidebar={() => {}} />
</div>

<style>
    .app-root {
        background-color: var(--bg-main);
        color: var(--fg-main);
    }

    .ribbon-container::-webkit-scrollbar {
        display: none;
    }

    .pane {
        flex-shrink: 0;
    }

    .project-row {
        background: var(--bg-main);
        border: 1px solid var(--border);
    }

    .text-accent { color: var(--accent); }
</style>
