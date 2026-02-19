<script lang="ts">
    import { onMount } from 'svelte';
    import { appState } from '$lib/stores/app.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import MarkdownPane from '$lib/components/MarkdownPane.svelte';
    import VirtualKeyboard from '$lib/components/VirtualKeyboard.svelte';
    import TopBar from '$lib/components/TopBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import ConnectionLogs from '$lib/components/ConnectionLogs.svelte';
    import { Menu, Monitor } from 'lucide-svelte';
    import { platform } from '@tauri-apps/plugin-os';

    let sidebarOpen = $state(false);
    let isLinux = $state(false);
    let ribbonContainer: HTMLDivElement;

    onMount(async () => {
        await appState.fetchAllProjects();
        const osPlatform = await platform();
        isLinux = osPlatform === 'linux';
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
<main class="container-fluid" onclick={closeSidebar}>
    {#if isLinux}
        <!-- Prevent clicks on the TopBar from closing the sidebar immediately -->
        <div onclick={(e) => e.stopPropagation()}>
            <TopBar onToggleSidebar={toggleSidebar} onScrollToPane={scrollToPane} />
        </div>
    {/if}

    <div class="grid" style="flex: 1; overflow: hidden; position: relative;">
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
                    <div class="welcome-screen">
                        <article onclick={(e) => e.stopPropagation()}>
                            <header>
                                <h1 class="title-font lowercase" style="color: var(--pico-primary); margin-bottom: 0;">zelland</h1>
                                <small class="secondary">mobile command center</small>
                            </header>
                            
                            <p>Select a host or session from the sidebar.</p>
                            
                            {#if !sidebarOpen}
                                <footer>
                                    <button class="outline contrast" onclick={(e) => { e.stopPropagation(); toggleSidebar(); }} style="width: auto; margin: 0 auto 1rem auto; display: flex; align-items: center; gap: 0.5rem;">
                                        <Menu size={16} /> Open Sidebar
                                    </button>

                                    {#if appState.recentSessions.length > 0}
                                        <div class="recent-sessions">
                                            <p><small class="secondary uppercase">Recent Sessions</small></p>
                                            <div class="grid">
                                                {#each appState.recentSessions as session}
                                                    <button class="outline secondary btn-sm" onclick={() => connectSession(session.id)}>
                                                        <Monitor size={14} /> {session.label}
                                                    </button>
                                                {/each}
                                            </div>
                                        </div>
                                    {/if}
                                </footer>
                            {/if}
                        </article>
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

    <!-- Bottom Area (Shortcut Bar / Virtual Keyboard) - Only on non-Linux (Mobile) -->
    {#if !isLinux}
        <div onclick={(e) => e.stopPropagation()}>
            <VirtualKeyboard onToggleSidebar={toggleSidebar} />
        </div>
    {/if}
</main>

<style>
    .ribbon-container::-webkit-scrollbar {
        display: none;
    }

    .welcome-screen {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        height: 100%;
        padding: 2rem;
        background: radial-gradient(circle at center, var(--pico-form-element-background-color) 0%, var(--pico-background-color) 100%);
    }

    .welcome-screen article {
        width: 100%;
        max-width: 400px;
        text-align: center;
    }
    
    .secondary {
        color: var(--fg-dim);
    }

    .recent-sessions {
        margin-top: 1.5rem;
        padding-top: 1rem;
        border-top: 1px solid var(--pico-border-color);
    }

    .uppercase {
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .btn-sm {
        padding: 0.25rem 0.5rem;
        font-size: 0.75rem;
        margin-bottom: 0;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 0.4rem;
    }
</style>
