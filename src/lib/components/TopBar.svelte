<script lang="ts">
    import { Menu, X, FileText } from 'lucide-svelte';
    import { appState } from '$lib/stores/app.svelte';
    import { invoke } from '@tauri-apps/api/core';

    let { onToggleSidebar, onScrollToPane } = $props<{ 
        onToggleSidebar: () => void,
        onScrollToPane: (index: number) => void
    }>();

    function sendKey(seq: string) {
        if (appState.activeSessionId) {
            appState.writeInput(appState.activeSessionId, Array.from(new TextEncoder().encode(seq)));
        }
    }

    function sendZellijTab(n: number) {
        if (appState.activeSessionId) {
            appState.runZellijAction(appState.activeSessionId, `go-to-tab ${n}`);
            onScrollToPane(0);
        }
    }

    async function closeWindow() {
        if (appState.activeSessionId) {
            appState.closeActiveSession();
        } else {
            // If no session active, close the actual application window
            await invoke('close_window');
        }
    }

    let sessionName = $derived.by(() => {
        if (!appState.activeSessionId) return '';
        const session = appState.sessions.find(s => s.id === appState.activeSessionId);
        return session ? session.label : '';
    });
</script>

<div class="top-bar" data-tauri-drag-region>
    <div class="left-group" data-tauri-drag-region>
        <button class="ghost-btn icon-btn key-unit" onclick={(e) => { e.stopPropagation(); onToggleSidebar(); }} title="Menu">
            <Menu size={18} />
        </button>
        <span class="session-label" onclick={() => onScrollToPane(0)} style="cursor: pointer;">{sessionName}</span>
    </div>

    <div class="center-group" data-tauri-drag-region>
        <div role="group" class="tab-group">
            <!-- Zellij Tabs -->
            <button class="outline contrast topbar-btn wide-unit" onclick={() => sendZellijTab(1)}>1</button>
            <button class="outline contrast topbar-btn wide-unit" onclick={() => sendZellijTab(2)}>2</button>
            <button class="outline contrast topbar-btn wide-unit" onclick={() => sendZellijTab(3)}>3</button>
            
            <!-- Markdown Nav Buttons -->
            {#each appState.openMarkdownFiles as file, i}
                {#if appState.loadedFiles[file]}
                    <button 
                        class="outline contrast topbar-btn file-nav-btn" 
                        onclick={() => onScrollToPane(i + 1)}
                        title="View {file}"
                    >
                        <span class="file-icon"><FileText size={12} /></span>
                        {file.split('/').pop()?.replace('.md', '')}
                    </button>
                {/if}
            {/each}
        </div>
    </div>

    <div class="right-group" data-tauri-drag-region>
        <button class="ghost-btn icon-btn key-unit close-btn" onclick={closeWindow} title="Close Session">
            <X size={18} />
        </button>
    </div>
</div>

<style>
    .top-bar {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.25rem 0.5rem;
        background-color: var(--bg-keyboard);
        border-bottom: 1px solid var(--pico-border-color);
        height: 3.5rem;
    }

    .left-group, .right-group {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        flex: 1;
    }

    .right-group {
        justify-content: flex-end;
    }

    .center-group {
        flex: 0;
    }

    .session-label {
        font-size: 1.1rem;
        color: var(--pico-color);
        font-weight: bold;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        margin-left: 0.5rem;
    }

    .key-unit {
        width: 2.2rem;
        height: 2.2rem;
        min-width: 2.2rem;
        padding: 0 !important;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.75rem;
        margin-bottom: 0 !important;
        line-height: 1;
    }

    .wide-unit {
        width: 4.4rem;
        height: 2.2rem;
        min-width: 4.4rem;
        padding: 0 !important;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.85rem;
        margin-bottom: 0 !important;
        line-height: 1;
        border-width: 1px;
    }

    .file-nav-btn {
        padding: 0 0.75rem !important;
        text-transform: uppercase;
        font-weight: bold;
        gap: 0.4rem;
    }

    .file-icon {
        display: flex;
        color: var(--pico-primary);
    }

    .topbar-btn {
        background-color: #2a2b3d;
        border-color: #4a4b5d;
        color: #c0caf5;
        height: 2.2rem;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.75rem;
    }

    .topbar-btn:active {
        background-color: var(--pico-primary);
        color: white;
    }

    .ghost-btn {
        background: transparent;
        border: none;
        color: var(--pico-muted-color);
        padding: 0;
        cursor: pointer;
    }
    .ghost-btn:hover {
        color: var(--pico-primary);
        background: rgba(255,255,255,0.05);
    }

    .close-btn:hover {
        color: var(--error) !important;
    }

    .tab-group {
        margin-bottom: 0 !important;
        display: flex;
        gap: 2px;
    }
</style>
