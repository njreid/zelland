<script lang="ts">
    import { appState } from '$lib/stores/app.svelte';

    const SOURCE_LABELS: Record<number, string> = {
        0: 'User', 1: 'Claude Code', 2: 'Gemini CLI', 3: 'Zellij'
    };
    const SOURCE_COLOURS: Record<number, string> = {
        0: 'bg-zinc-600', 1: 'bg-blue-600', 2: 'bg-green-600', 3: 'bg-purple-600'
    };

    const notif = $derived(appState.lastAgentNotification);
    const label = $derived(notif ? (SOURCE_LABELS[notif.source] ?? 'Agent') : '');
    const colour = $derived(notif ? (SOURCE_COLOURS[notif.source] ?? 'bg-zinc-600') : '');

    async function navigate() {
        if (!notif?.session_name) return;
        await appState.navigateToSession(notif.session_name, notif.tab_index);
        appState.dismissAgentNotification();
    }
</script>

{#if notif}
<div class="toast-overlay">
    <div class="toast-header">
        <span class="source-badge {colour}">{label}</span>
        <span class="toast-title">{notif.title}</span>
        <button
            onclick={() => appState.dismissAgentNotification()}
            class="dismiss-btn"
            aria-label="Dismiss"
        >✕</button>
    </div>
    {#if notif.question}
        <p class="toast-question">{notif.question}</p>
    {/if}
    {#if notif.session_name}
        <button onclick={navigate} class="navigate-btn">
            Switch to {notif.session_name}
        </button>
    {/if}
</div>
{/if}

<style>
    .toast-overlay {
        position: fixed;
        bottom: 1rem;
        left: 1rem;
        right: 1rem;
        z-index: 50;
        border-radius: 0.75rem;
        border: 1px solid rgba(255,255,255,0.1);
        background: rgba(24, 24, 27, 0.95);
        padding: 1rem;
        box-shadow: 0 25px 50px -12px rgba(0,0,0,0.5);
        backdrop-filter: blur(8px);
    }

    .toast-header {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        margin-bottom: 0.5rem;
    }

    .source-badge {
        border-radius: 0.25rem;
        padding: 0.125rem 0.5rem;
        font-size: 0.75rem;
        font-weight: 500;
        color: white;
        flex-shrink: 0;
    }

    .bg-zinc-600  { background: #52525b; }
    .bg-blue-600  { background: #2563eb; }
    .bg-green-600 { background: #16a34a; }
    .bg-purple-600 { background: #9333ea; }

    .toast-title {
        font-size: 0.875rem;
        font-weight: 600;
        color: white;
        flex: 1;
        min-width: 0;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .dismiss-btn {
        margin-left: auto;
        color: #a1a1aa;
        background: none;
        border: none;
        cursor: pointer;
        padding: 0.125rem 0.25rem;
        font-size: 0.875rem;
        flex-shrink: 0;
    }

    .dismiss-btn:hover { color: white; }

    .toast-question {
        font-size: 0.875rem;
        color: #d4d4d8;
        margin: 0 0 0.75rem;
        line-height: 1.5;
    }

    .navigate-btn {
        width: 100%;
        border-radius: 0.5rem;
        background: #2563eb;
        padding: 0.5rem;
        font-size: 0.875rem;
        font-weight: 500;
        color: white;
        border: none;
        cursor: pointer;
    }

    .navigate-btn:hover  { background: #3b82f6; }
    .navigate-btn:active { background: #1d4ed8; }
</style>
