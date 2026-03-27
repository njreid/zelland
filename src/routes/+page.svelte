<script lang="ts">
    import { onMount } from 'svelte';
    import { appState } from '$lib/stores/app.svelte';
    import Terminal from '$lib/components/Terminal.svelte';
    import MarkdownPane from '$lib/components/MarkdownPane.svelte';
    import TopBar from '$lib/components/TopBar.svelte';
    import Sidebar from '$lib/components/Sidebar.svelte';
    import ConnectionLogs from '$lib/components/ConnectionLogs.svelte';
    import { Menu, Terminal as TerminalIcon, X, Server, Settings, RotateCcw, Key, Trash2, Copy } from 'lucide-svelte';
    import type { DaemonRecentSession } from '$lib/stores/app.svelte';
    import { platform } from '@tauri-apps/plugin-os';
    import { handleKbInput } from '$lib/utils/kb-input';
    import { timeAgo } from '$lib/utils/time-ago';
    import AgentNotificationToast from '$lib/components/AgentNotificationToast.svelte';

    let sidebarOpen = $state(false);
    let isLinux = $state(false);
    let ribbonContainer: HTMLDivElement;
    let currentPaneIndex = $state(0);

    // Android modal forms (triggered by native sidebar buttons)
    type AndroidModal = 'addHost' | 'addSession' | 'settings' | null;
    let androidModal = $state<AndroidModal>(null);

    // Add Host form
    let newHostAddress = $state('');
    let newHostUser = $state('');
    let newHostPass = $state('');

    // Add Session form
    let newSessionName = $state('');
    let newSessionHost = $state('');
    let newSessionUser = $state('');
    let newSessionPass = $state('');
    let newSessionAuthMethod = $state<'Password' | 'Key' | 'PrivateKey'>('Password');
    let newSessionKeyId = $state('');
    let newSessionPrivateKeyPath = $state('');

    // Key generation in settings
    let newKeyLabel = $state('');
    let showKeyForm = $state(false);

    function closeModal() { androidModal = null; }

    async function handleAddHost() {
        if (newHostAddress && newHostUser) {
            await appState.addHost(newHostAddress, newHostAddress, newHostUser, newHostPass);
            newHostAddress = ''; newHostUser = ''; newHostPass = '';
            closeModal();
        }
    }

    async function handleAddSession() {
        if (newSessionName && newSessionHost && newSessionUser) {
            await appState.addSession(
                newSessionName, newSessionHost, newSessionUser, 'ssh', newSessionName,
                newSessionAuthMethod === 'Password' ? newSessionPass : undefined,
                newSessionAuthMethod === 'Key' ? newSessionKeyId : undefined,
                newSessionAuthMethod === 'PrivateKey' ? newSessionPrivateKeyPath : undefined
            );
            newSessionName = ''; newSessionHost = ''; newSessionUser = ''; newSessionPass = '';
            newSessionPrivateKeyPath = ''; newSessionKeyId = '';
            closeModal();
            const sessions = appState.sessions;
            const newSession = sessions[sessions.length - 1];
            if (newSession) {
                await appState.connectSession(newSession.id);
                appState.triggerTerminalFocus();
                appState.triggerTerminalResize();
            }
        }
    }

    onMount(async () => {
        await appState.fetchAllProjects();
        const osPlatform = await platform();
        isLinux = osPlatform === 'linux';

        if (!isLinux) {
            window.addEventListener('kb-sidebar-toggle', () => {
                (window as any).SidebarNative?.openDrawer();
            });
            window.addEventListener('kb-go-to-tab', (e) => {
                const { tab } = (e as CustomEvent<{ tab: number }>).detail;
                if (appState.activeSessionId) {
                    appState.runZellijAction(appState.activeSessionId, `go-to-tab ${tab}`);
                }
            });
            // Native sidebar button events
            window.addEventListener('native-reload-terminal', () => {
                if (appState.activeSessionId) appState.restartActiveSession();
            });
            window.addEventListener('native-add-host',    () => { androidModal = 'addHost'; });
            window.addEventListener('native-add-session', () => { androidModal = 'addSession'; });
            window.addEventListener('native-settings',    () => { androidModal = 'settings'; });
            window.addEventListener('native-connect-session', (e) => {
                const { id } = (e as CustomEvent<{ id: string }>).detail;
                connectSession(id);
            });
            // Drawer closed: restore keybar visibility
            window.addEventListener('native-drawer-closed', () => {
                const visible = !!appState.activeSessionId && currentPaneIndex === 0;
                (window as any).KeybarNative?.setVisible(visible);
            });
        } else {
            window.addEventListener('kb-sidebar-toggle', () => { toggleSidebar(); });
            window.addEventListener('kb-go-to-tab', (e) => {
                const { tab } = (e as CustomEvent<{ tab: number }>).detail;
                if (appState.activeSessionId) {
                    appState.runZellijAction(appState.activeSessionId, `go-to-tab ${tab}`);
                }
            });
        }

        ribbonContainer?.addEventListener('scroll', () => {
            const idx = Math.round(ribbonContainer.scrollLeft / (ribbonContainer.clientWidth || 1));
            currentPaneIndex = idx;
        });

        (window as any).__navigateToSession = (sessionName: string) => {
            appState.navigateToSession(sessionName, 0);
            sidebarOpen = false;
        };
    });

    // Push session/host data to native sidebar whenever state changes.
    $effect(() => {
        if (!isLinux) {
            const data = {
                sessions: appState.sessions.map(s => ({
                    id: s.id,
                    label: s.label,
                    status: s.status
                })),
                hosts: appState.hosts.map(h => ({
                    id: h.id,
                    label: h.label,
                    reachable: h.reachable
                })),
                activeSessionId: appState.activeSessionId ?? ''
            };
            (window as any).SidebarNative?.updateData(JSON.stringify(data));
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

    function scrollToPane(index: number) {
        if (ribbonContainer) {
            const paneWidth = ribbonContainer.clientWidth;
            ribbonContainer.scrollTo({ left: index * paneWidth, behavior: 'smooth' });
        }
    }

    $effect(() => {
        if (appState.navigationTrigger >= 0) scrollToPane(appState.navigationTrigger);
    });

    $effect(() => {
        if (appState.activeSessionId) {
            sidebarOpen = false;
            scrollToPane(0);
        }
    });

    $effect(() => {
        const visible = !!appState.activeSessionId && currentPaneIndex === 0 && !sidebarOpen;
        (window as any).KeybarNative?.setVisible(visible);
    });

    $effect(() => {
        (window as any).TerminalNative?.setVisible(currentPaneIndex === 0 && !!appState.activeSessionId);
    });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<main class="container-fluid" onclick={closeSidebar}>
    {#if isLinux}
        <div onclick={(e) => e.stopPropagation()}>
            <TopBar onToggleSidebar={toggleSidebar} onScrollToPane={scrollToPane} />
        </div>
    {/if}

    <div style="display: flex; flex: 1; overflow: hidden; position: relative;">
        {#if isLinux && sidebarOpen}
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
                                            <div class="session-card" onclick={() => connectDaemonSession(entry)} role="button">
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
                                <button class="outline contrast menu-btn" onclick={(e) => {
                                    e.stopPropagation();
                                    if (isLinux) toggleSidebar();
                                    else (window as any).SidebarNative?.openDrawer();
                                }}>
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

<!-- Android form modals (triggered by native sidebar buttons) -->
{#if !isLinux && androidModal !== null}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="modal-backdrop" onclick={closeModal}>
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="modal-sheet" onclick={(e) => e.stopPropagation()}>
            {#if androidModal === 'addHost'}
                <div class="modal-header">
                    <strong>New Host</strong>
                    <button type="button" class="modal-close" onclick={closeModal}><X size={18} /></button>
                </div>
                <form onsubmit={(e) => { e.preventDefault(); handleAddHost(); }} class="modal-form">
                    <input type="text"     placeholder="Address (IP/FQDN)" bind:value={newHostAddress} required />
                    <input type="text"     placeholder="Username"           bind:value={newHostUser}    required />
                    <input type="password" placeholder="Password (optional)" bind:value={newHostPass} />
                    <div class="modal-actions">
                        <button type="submit">Add Host</button>
                        <button type="button" class="secondary outline" onclick={closeModal}>Cancel</button>
                    </div>
                </form>

            {:else if androidModal === 'addSession'}
                <div class="modal-header">
                    <strong>New Session</strong>
                    <button type="button" class="modal-close" onclick={closeModal}><X size={18} /></button>
                </div>
                <form onsubmit={(e) => { e.preventDefault(); handleAddSession(); }} class="modal-form">
                    <input type="text" placeholder="Session Name"  bind:value={newSessionName} required />
                    <input type="text" placeholder="Host Address"  bind:value={newSessionHost} required />
                    <input type="text" placeholder="Username"      bind:value={newSessionUser} required />
                    <select bind:value={newSessionAuthMethod}>
                        <option value="Password">Password</option>
                        <option value="Key">SSH Identity</option>
                        <option value="PrivateKey">Private Key File</option>
                    </select>
                    {#if newSessionAuthMethod === 'Password'}
                        <input type="password" placeholder="Password" bind:value={newSessionPass} />
                    {:else if newSessionAuthMethod === 'Key'}
                        <select bind:value={newSessionKeyId} required>
                            <option value="" disabled selected>Select Identity…</option>
                            {#each appState.sshKeys as key}
                                <option value={key.id}>{key.label}</option>
                            {/each}
                        </select>
                    {:else if newSessionAuthMethod === 'PrivateKey'}
                        <input type="text" placeholder="~/.ssh/id_ed25519" bind:value={newSessionPrivateKeyPath} required />
                    {/if}
                    <div class="modal-actions">
                        <button type="submit">Create &amp; Connect</button>
                        <button type="button" class="secondary outline" onclick={closeModal}>Cancel</button>
                    </div>
                </form>

            {:else if androidModal === 'settings'}
                <div class="modal-header">
                    <strong>Settings</strong>
                    <button type="button" class="modal-close" onclick={closeModal}><X size={18} /></button>
                </div>
                <div class="modal-form">
                    <p class="settings-label">TERMINAL FONT</p>
                    <div class="settings-row">
                        <label>
                            Size (px)
                            <input type="number" value={appState.terminalFontSize}
                                onchange={(e) => appState.setTerminalFontSize(parseInt(e.currentTarget.value) || 14)} />
                        </label>
                        <label>
                            Weight
                            <select value={appState.terminalFontWeight}
                                onchange={(e) => appState.setTerminalFontWeight(e.currentTarget.value)}>
                                {#each ['normal','bold','100','200','300','400','500','600','700','800','900'] as w}
                                    <option value={w}>{w}</option>
                                {/each}
                            </select>
                        </label>
                    </div>

                    <p class="settings-label" style="margin-top:1rem;">DOCUMENT FONT</p>
                    <div class="settings-row">
                        <label>
                            Size (px)
                            <input type="number" value={appState.markdownFontSize}
                                onchange={(e) => appState.setMarkdownFontSize(parseInt(e.currentTarget.value) || 16)} />
                        </label>
                        <label>
                            Weight
                            <select value={appState.markdownFontWeight}
                                onchange={(e) => appState.setMarkdownFontWeight(e.currentTarget.value)}>
                                {#each ['100','200','300','400','500','600','700','800','900'] as w}
                                    <option value={w}>{w}</option>
                                {/each}
                            </select>
                        </label>
                    </div>

                    <p class="settings-label" style="margin-top:1rem;">SSH IDENTITIES</p>
                    {#if appState.sshKeys.length === 0}
                        <p class="secondary" style="font-size:0.85rem;">No keys generated yet.</p>
                    {:else}
                        {#each appState.sshKeys as key}
                            <div class="key-row">
                                <Key size={12} />
                                <span class="key-label">{key.label}</span>
                                <button class="icon-btn-sm" title="Copy" onclick={() => navigator.clipboard.writeText('ssh-ed25519 ' + key.public_key)}><Copy size={12} /></button>
                                <button class="icon-btn-sm danger" title="Delete" onclick={() => appState.deleteSshKey(key.id)}><Trash2 size={12} /></button>
                            </div>
                        {/each}
                    {/if}

                    {#if showKeyForm}
                        <form class="key-gen-row" onsubmit={(e) => {
                            e.preventDefault();
                            if (newKeyLabel.trim()) { appState.generateSshKey(newKeyLabel.trim()); newKeyLabel = ''; showKeyForm = false; }
                        }}>
                            <input type="text" placeholder="Key label" bind:value={newKeyLabel} required />
                            <button type="submit"><Key size={14} /> Generate</button>
                        </form>
                    {:else}
                        <button class="outline contrast" style="width:auto;margin-top:0.5rem;font-size:0.85rem;" onclick={() => { showKeyForm = true; }}>+ New Key</button>
                    {/if}

                    <div class="modal-actions" style="margin-top:1.5rem;">
                        <button onclick={closeModal}>Done</button>
                    </div>
                </div>
            {/if}
        </div>
    </div>
{/if}

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

    .welcome-header { margin-bottom: 0.5rem; }
    .secondary { color: var(--fg-dim); }

    .section-label {
        font-size: 0.65rem;
        font-weight: 700;
        letter-spacing: 0.1em;
        color: var(--fg-dim);
        margin: 1rem 0 0.6rem;
    }

    .recent-sessions { width: 100%; }

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

    .card-icon { color: var(--pico-primary); display: flex; flex-shrink: 0; }

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

    .card-footer { margin-top: auto; }

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

    /* ---- Android modal ---- */
    .modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.6);
        z-index: 200;
        display: flex;
        align-items: flex-end;
    }

    .modal-sheet {
        width: 100%;
        max-height: 85vh;
        background: var(--pico-card-background-color, #1a1b26);
        border-radius: 16px 16px 0 0;
        overflow-y: auto;
        padding-bottom: env(safe-area-inset-bottom, 0px);
    }

    .modal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 1rem 1.25rem 0.5rem;
        border-bottom: 1px solid var(--pico-border-color);
    }

    .modal-close {
        background: none;
        border: none;
        padding: 0.25rem;
        margin: 0;
        display: flex;
        color: var(--fg-dim);
        cursor: pointer;
    }

    .modal-form {
        padding: 1rem 1.25rem;
    }

    .modal-form input,
    .modal-form select {
        margin-bottom: 0.65rem;
        font-size: 1rem;
    }

    .modal-actions {
        display: flex;
        gap: 0.5rem;
        margin-top: 0.5rem;
    }

    .modal-actions button {
        flex: 1;
        margin-bottom: 0;
    }

    .settings-label {
        font-size: 0.7rem;
        font-weight: 700;
        letter-spacing: 0.08em;
        color: var(--fg-dim);
        margin: 0 0 0.5rem;
    }

    .settings-row {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 0.5rem;
    }

    .settings-row label {
        font-size: 0.8rem;
    }

    .settings-row input,
    .settings-row select {
        font-size: 0.85rem;
        padding: 0.4rem;
        margin-bottom: 0;
    }

    .key-row {
        display: flex;
        align-items: center;
        gap: 0.4rem;
        padding: 0.35rem 0;
        border-bottom: 1px solid var(--pico-border-color);
    }

    .key-label {
        flex: 1;
        font-size: 0.85rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .icon-btn-sm {
        background: none;
        border: none;
        padding: 0.2rem;
        margin: 0;
        display: flex;
        color: var(--fg-dim);
        cursor: pointer;
    }

    .icon-btn-sm.danger:hover { color: var(--error, #f7768e); }

    .key-gen-row {
        display: flex;
        gap: 0.4rem;
        margin-top: 0.5rem;
    }

    .key-gen-row input {
        flex: 1;
        margin-bottom: 0;
        font-size: 0.9rem;
    }

    .key-gen-row button {
        white-space: nowrap;
        font-size: 0.85rem;
        display: flex;
        align-items: center;
        gap: 0.3rem;
        margin-bottom: 0;
    }
</style>
