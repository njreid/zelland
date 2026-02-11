<script lang="ts">
    import { appState } from '$lib/stores/app.svelte';
    import { fade } from 'svelte/transition';

    let showLogs = $state(false);

    $effect(() => {
        if (appState.logs.length > 0) {
            showLogs = true;
            const timer = setTimeout(() => showLogs = false, 5000);
            return () => clearTimeout(timer);
        }
    });
</script>

{#if showLogs && appState.logs.length > 0}
    <div class="logs-container" transition:fade>
        {#each appState.logs.slice(-5) as log}
            <div class="log-entry {log.type}">
                {log.message}
            </div>
        {/each}
    </div>
{/if}

<style>
    .logs-container {
        position: absolute;
        bottom: 4rem; /* Above shortcut bar */
        right: 1rem;
        width: 300px;
        background: rgba(0, 0, 0, 0.8);
        border: 1px solid var(--pico-border-color);
        border-radius: 0.25rem;
        padding: 0.5rem;
        pointer-events: none;
        z-index: 1000;
        font-size: 0.8rem;
    }

    .log-entry {
        margin-bottom: 0.25rem;
        word-wrap: break-word;
    }

    .log-entry.info {
        color: var(--pico-color);
    }

    .log-entry.error {
        color: var(--error);
    }
</style>
