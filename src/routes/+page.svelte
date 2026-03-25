<script lang="ts">
    import { onMount } from 'svelte';
    import { appState } from '$lib/stores/app.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import MarkdownPane from '$lib/components/MarkdownPane.svelte';
    import TopBar from '$lib/components/TopBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import ConnectionLogs from '$lib/components/ConnectionLogs.svelte';
    import { Menu, Terminal as TerminalIcon } from 'lucide-svelte';
    import type { DaemonRecentSession } from '$lib/stores/app.svelte';
    import { platform } from '@tauri-apps/plugin-os';
    import { handleKbInput } from '$lib/utils/kb-input';
    import AgentNotificationToast from '$lib/components/AgentNotificationToast.svelte';

    let sidebarOpen = $state(false);
    let isLinux = $state(false);
    let ribbonContainer: HTMLDivElement;

    onMount(async () => {
        await appState.fetchAllProjects();
        const osPlatform = await platform();
        isLinux = osPlatform === 'linux';

        if (!isLinux) {
            window.addEventListener('kb-input', (e) => {
                const { seq } = (e as CustomEvent<{ seq: string }>).detail;
                handleKbInput(seq, appState.activeSessionId, (id, bytes) => {
                    appState.writeInput(id, bytes);
                });
            });
            window.addEventListener('kb-sidebar-toggle', () => {
                toggleSidebar();
            });
            window.addEventListener('kb-go-to-tab', (e) => {
                const { tab } = (e as CustomEvent<{ tab: number }>).detail;
                if (appState.activeSessionId) {
                    appState.runZellijAction(appState.activeSessionId, `go-to-tab ${tab}`);
                }
            });
        }
    });

    function toggleSidebar() {
        sidebarOpen = !sidebarOpen;
    }

    function closeSidebar() {
        if (sidebarOpen) sidebarOpen = false;
    }

    async function connectSession(sessionId: string) {
        await appState.connectSession(sessionId);
        appState.triggerTerminalFocus();
        appState.triggerTerminalResize();
    }

    async function connectDaemonSession(entry: DaemonRecentSession) {
        await appState.connectDaemonSession(entry);
        appState.triggerTerminalFocus();
        appState.triggerTerminalResize();
    }

    function timeAgo(isoString: string): string {
        const ms = Date.now() - new Date(isoString).getTime();
        const s = Math.floor(ms / 1000);
        if (s < 60) return 'just now';
        const m = Math.floor(s / 60);
        if (m < 60) return `${m}m ago`;
        const h = Math.floor(m / 60);
        if (h < 24) return `${h}h ago`;
        const d = Math.floor(h / 24);
        return `${d}d ago`;
    }

    function scrollToPane(index: number) {
        if (ribbonContainer) {
            const paneWidth = ribbonContainer.clientWidth;
            ribbonContainer.scrollTo({
                left: index * paneWidth,
                behavior: 'smooth'
            });
        }
    }

    // Reactive navigation
    $effect(() => {
        if (appState.navigationTrigger >= 0) {
            scrollToPane(appState.navigationTrigger);
        }
    });

    // Close sidebar when a session becomes active
    $effect(() => {
        if (appState.activeSessionId) {
            sidebarOpen = false;
            scrollToPane(0);
        }
    });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="container-fluid" onclick={closeSidebar}>
    {#if isLinux}
        <!-- Prevent clicks on the TopBar from closing the sidebar immediately -->
        <div onclick={(e) => e.stopPropagation()}>
            <TopBar onToggleSidebar={toggleSidebar} onScrollToPane={scrollToPane} />
        </div>
    {/if}

    <div style="display: flex; flex: 1; overflow: hidden; position: relative;">
        {#if sidebarOpen}
            <!-- Prevent clicks inside the sidebar from closing it -->
            <div onclick={(e) => e.stopPropagation()}>
                <Sidebar />
            </div>
        {/if}

        <div 
            bind:this={ribbonContainer}
            class="ribbon-container scrollbar-hide" 
            style="flex: 1; overflow-x: auto; overflow-y: hidden; scroll-snap-type: x mandatory; display: flex;"
        >
            <!-- Pane 0: Terminal -->
            <section class="pane" style="border-left: none; position: relative;">
                {#if appState.activeSessionId}
                    {#key appState.activeSessionId}
                        <Terminal tabId={appState.activeSessionId} />
                    {/key}
                {:else}
                    <div class="welcome-screen" onclick={(e) => e.stopPropagation()}>
                        <div class="welcome-inner">
                            <div class="welcome-header">
                                <h1 class="title-font lowercase" style="color: var(--pico-primary); margin-bottom: 0.1rem;">zelland</h1>
                                <small class="secondary">mobile command center</small>
                            </div>

                            {#if appState.daemonRecentSessions.length > 0}
                                <div class="recent-sessions">
                                    <p class="section-label">RECENT</p>
                                    <div class="session-grid">
                                        {#each appState.daemonRecentSessions as entry}
                                            <!-- svelte-ignore a11y_interactive_supports_focus -->
                                            <div
                                                class="session-card"
                                                onclick={() => connectDaemonSession(entry)}
                                                role="button"
                                            >
                                                <div class="card-top">
                                                    <span class="card-icon"><TerminalIcon size={14} /></span>
                                                    <span class="card-name">{entry.sessionName}</span>
                                                    <span class="card-host">{entry.hostLabel || entry.hostAddress}</span>
                                                </div>
                                                {#if entry.readmeExcerpt}
                                                    <p class="card-excerpt">{entry.readmeExcerpt}</p>
                                                {/if}
                                                <div class="card-footer">
                                                    <span class="card-time">{timeAgo(entry.connectedAt)}</span>
                                                </div>
                                            </div>
                                        {/each}
                                    </div>
                                </div>
                            {:else}
                                <p class="secondary" style="margin-top: 1.5rem; font-size: 0.9rem;">
                                    Open the sidebar to connect to a session.
                                </p>
                            {/if}

                            {#if !sidebarOpen}
                                <button class="outline contrast menu-btn" onclick={(e) => { e.stopPropagation(); toggleSidebar(); }}>
                                    <Menu size={16} /> Sidebar
                                </button>
                            {/if}
                        </div>
                    </div>
                {/if}
                <ConnectionLogs />
            </section>

            <!-- Markdown Panes -->
            {#each appState.openMarkdownFiles as file}
                <section class="pane">
                    <MarkdownPane filename={file} />
                </section>
            {/each}
        </div>
    </div>

    <AgentNotificationToast />
</main>

<style>
    .ribbon-container::-webkit-scrollbar {
        display: none;
    }

    .welcome-screen {
        display: flex;
        align-items: center;
        justify-content: center;
        height: 100%;
        background: radial-gradient(circle at 40% 40%, var(--pico-form-element-background-color) 0%, var(--pico-background-color) 100%);
        overflow-y: auto;
    }

    .welcome-inner {
        width: 100%;
        max-width: 480px;
        padding: 2rem 1.5rem;
        display: flex;
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
    }

    .welcome-header {
        margin-bottom: 0.5rem;
    }

    .secondary {
        color: var(--fg-dim);
    }

    .section-label {
        font-size: 0.65rem;
        font-weight: 700;
        letter-spacing: 0.1em;
        color: var(--fg-dim);
        margin: 1rem 0 0.6rem;
    }

    .recent-sessions {
        width: 100%;
    }

    .session-grid {
        display: grid;
        grid-template-columns: repeat(2, 1fr);
        gap: 0.6rem;
    }

    @media (max-width: 400px) {
        .session-grid { grid-template-columns: 1fr; }
    }

    .session-card {
        background: var(--pico-card-background-color, #1e1f2e);
        border: 1px solid var(--pico-border-color);
        border-radius: 6px;
        padding: 0.85rem 0.9rem;
        cursor: pointer;
        display: flex;
        flex-direction: column;
        gap: 0.45rem;
        transition: border-color 0.15s, background 0.15s;
        min-height: 5.5rem;
    }

    .session-card:hover {
        border-color: var(--pico-primary);
        background: color-mix(in srgb, var(--pico-primary) 6%, var(--pico-card-background-color, #1e1f2e));
    }

    .session-card:active {
        background: color-mix(in srgb, var(--pico-primary) 14%, var(--pico-card-background-color, #1e1f2e));
    }

    .card-top {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        flex-wrap: wrap;
    }

    .card-icon {
        color: var(--pico-primary);
        display: flex;
        flex-shrink: 0;
    }

    .card-name {
        font-size: 0.95rem;
        font-weight: 600;
        color: var(--pico-color);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        flex: 1;
        min-width: 0;
    }

    .card-host {
        font-size: 0.65rem;
        color: var(--fg-dim);
        background: var(--pico-form-element-background-color);
        padding: 0.1rem 0.35rem;
        border-radius: 3px;
        white-space: nowrap;
        flex-shrink: 0;
    }

    .card-excerpt {
        font-size: 0.72rem;
        color: var(--fg-dim);
        line-height: 1.45;
        margin: 0;
        display: -webkit-box;
        -webkit-line-clamp: 3;
        line-clamp: 3;
        -webkit-box-orient: vertical;
        overflow: hidden;
        flex: 1;
    }

    .card-footer {
        margin-top: auto;
    }

    .card-time {
        font-size: 0.65rem;
        color: var(--fg-dim);
        opacity: 0.7;
    }

    .menu-btn {
        margin-top: 1.5rem;
        width: auto;
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.45rem 1rem;
        font-size: 0.85rem;
    }
</style>
