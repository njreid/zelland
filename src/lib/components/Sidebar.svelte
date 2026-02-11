<script lang="ts">
    import { appState } from '$lib/stores/app.svelte';
    import { Plus, Server, Terminal, AlertCircle, Globe, Play, Monitor, X, Trash2, Circle, Settings } from 'lucide-svelte';

    let showAddHost = $state(false);
    let showAddSession = $state(false);
    let showSettings = $state(false);
    
    // Host form
    let newHostAddress = $state('');
    let newHostUser = $state('');
    let newHostPass = $state('');

    // Session form
    let newSessionName = $state('');
    let newSessionHost = $state('');
    let newSessionUser = $state('');
    let newSessionPass = $state('');
    let newSessionAuthMethod = $state<'Password' | 'Key'>('Password');
    let newSessionKeyId = $state('');

    async function handleAddHost() {
        if (newHostAddress && newHostUser) {
            await appState.addHost(newHostAddress, newHostAddress, newHostUser, newHostPass);
            resetHostForm();
        }
    }

    function resetHostForm() {
        newHostAddress = ''; newHostUser = ''; newHostPass = '';
        showAddHost = false;
    }

    async function handleAddSession() {
        if (newSessionName && newSessionHost && newSessionUser) {
            await appState.addSession(
                newSessionName, 
                newSessionHost, 
                newSessionUser, 
                'ssh', 
                newSessionName, 
                newSessionAuthMethod === 'Password' ? newSessionPass : undefined,
                newSessionAuthMethod === 'Key' ? newSessionKeyId : undefined
            );
            resetSessionForm();
            const sessions = appState.sessions;
            const newSession = sessions[sessions.length - 1];
            if (newSession) {
                await appState.connectSession(newSession.id);
                appState.triggerTerminalFocus();
                appState.triggerTerminalResize();
            }
        }
    }

    function resetSessionForm() {
        newSessionName = ''; newSessionHost = ''; newSessionUser = ''; newSessionPass = '';
        showAddSession = false;
    }

    function getSessionStatusColor(status: string) {
        switch (status) {
            case 'connected': return 'var(--mod-meta)';
            case 'connecting': return 'var(--mod-alt)';
            case 'error': return 'var(--error)';
            default: return 'var(--fg-dim)';
        }
    }
</script>

<aside class="sidebar">
    <nav>
        <ul>
            <li><strong class="title-font lowercase" style="color: var(--pico-primary);">zelland</strong></li>
        </ul>
    </nav>

    <!-- Forms -->
    {#if showAddHost}
        <article class="form-card">
            <header>
                <strong>New Host</strong>
                <a href="#" onclick={(e) => { e.preventDefault(); resetHostForm(); }} class="close"><X size={14} /></a>
            </header>
            <form onsubmit={(e) => { e.preventDefault(); handleAddHost(); }}>
                <input type="text" placeholder="Address (IP/FQDN)" bind:value={newHostAddress} aria-label="Address" required />
                <input type="text" placeholder="Username" bind:value={newHostUser} aria-label="Username" required />
                <input type="password" placeholder="Password (Optional)" bind:value={newHostPass} aria-label="Password" />
                <div class="grid">
                    <button type="submit">Add</button>
                    <button type="button" class="secondary outline" onclick={resetHostForm}>Cancel</button>
                </div>
            </form>
        </article>
    {/if}

    {#if showAddSession}
        <article class="form-card">
            <header>
                <strong>New Session</strong>
                <a href="#" onclick={(e) => { e.preventDefault(); resetSessionForm(); }} class="close"><X size={14} /></a>
            </header>
            <form onsubmit={(e) => { e.preventDefault(); handleAddSession(); }}>
                <input type="text" placeholder="Name" bind:value={newSessionName} aria-label="Name" required />
                <input type="text" placeholder="Host Address" bind:value={newSessionHost} aria-label="Host Address" required />
                <input type="text" placeholder="Username" bind:value={newSessionUser} aria-label="Username" required />
                
                <select bind:value={newSessionAuthMethod} aria-label="Auth Method">
                    <option value="Password">Password</option>
                    <option value="Key">SSH Identity</option>
                </select>

                {#if newSessionAuthMethod === 'Password'}
                    <input type="password" placeholder="Password" bind:value={newSessionPass} aria-label="Password" />
                {:else}
                    <select bind:value={newSessionKeyId} aria-label="SSH Identity" required>
                        <option value="" disabled selected>Select Identity...</option>
                        {#each appState.sshKeys as key}
                            <option value={key.id}>{key.label}</option>
                        {/each}
                    </select>
                {/if}

                <div class="grid">
                    <button type="submit">Create</button>
                    <button type="button" class="secondary outline" onclick={resetSessionForm}>Cancel</button>
                </div>
            </form>
        </article>
    {/if}

    {#if showSettings}
        <article class="form-card">
            <header>
                <strong>Settings</strong>
                <a href="#" onclick={(e) => { e.preventDefault(); showSettings = false; }} class="close"><X size={14} /></a>
            </header>
            <div class="p-4 pt-0">
                <div class="mb-4">
                    <label>
                        Terminal Font Size: {appState.terminalFontSize}px
                        <input 
                            type="range" 
                            min="8" 
                            max="32" 
                            value={appState.terminalFontSize} 
                            oninput={(e) => appState.setTerminalFontSize(parseInt(e.currentTarget.value))}
                        />
                    </label>
                </div>

                <div class="ssh-keys-section">
                    <div class="flex-row justify-between mb-2">
                        <small>SSH IDENTITIES</small>
                        <button class="outline contrast icon-only-tiny" onclick={() => appState.generateSshKey('My Phone')} title="Generate New Key">
                            <Plus size={14} />
                        </button>
                    </div>
                    
                    {#if appState.sshKeys.length === 0}
                        <p class="text-xs secondary">No keys generated yet.</p>
                    {:else}
                        <ul class="list-none p-0">
                            {#each appState.sshKeys as key}
                                <li class="key-item mb-2">
                                    <div class="flex-row justify-between">
                                        <span class="text-sm">{key.label}</span>
                                        <div class="flex-row gap-1">
                                            <button class="secondary icon-only-tiny" onclick={() => navigator.clipboard.writeText(key.public_key)} title="Copy Public Key">
                                                <Play size={10} />
                                            </button>
                                            <button class="secondary hover-error icon-only-tiny" onclick={() => appState.deleteSshKey(key.id)} title="Delete Key">
                                                <Trash2 size={10} />
                                            </button>
                                        </div>
                                    </div>
                                    <div class="text-xs secondary truncate" style="max-width: 180px;">{key.public_key}</div>
                                </li>
                            {/each}
                        </ul>
                    {/if}
                </div>
            </div>
        </article>
    {/if}

    <div class="tree-container scrollbar-hide">
        <!-- Sessions Section -->
        <small>SESSIONS</small>
        <ul class="list-none">
            {#each appState.sessions as session}
                <li>
                    <div class="session-row-container">
                        <a href="#" 
                           class={appState.activeSessionId === session.id ? 'primary' : 'secondary'}
                           onclick={(e) => { e.preventDefault(); appState.connectSession(session.id).then(() => { appState.triggerTerminalFocus(); appState.triggerTerminalResize(); }); }}
                           role="button"
                           class:outline={appState.activeSessionId !== session.id}
                           style="flex: 1;"
                        >
                            <div class="flex-row">
                                <Circle size={8} fill={getSessionStatusColor(session.status)} stroke="none" />
                                <Monitor size={14} /> {session.label}
                            </div>
                        </a>
                        <button class="outline contrast delete-btn" onclick={() => appState.removeSession(session.id)} aria-label="Delete Session">
                            <Trash2 size={14} />
                        </button>
                    </div>
                </li>
            {/each}
        </ul>

        <!-- Hosts Section -->
        <small class="mt-4 block">HOSTS</small>
        <ul class="list-none">
            {#each appState.hosts as host}
                <li>
                    <details>
                        <summary class="host-summary" title={host.error}>
                            <div class="flex-row">
                                {#if host.reachable}
                                    <Globe size={14} style="color: var(--mod-meta);" />
                                {:else}
                                    <AlertCircle size={14} style="color: var(--error);" />
                                {/if}
                                <span class={!host.reachable ? 'text-error' : ''}>{host.label}</span>
                            </div>
                            <div class="flex-row gap-1">
                                <a href="#" onclick={(e) => { e.preventDefault(); e.stopPropagation(); appState.fetchProjectsForHost(host.id); }} class="secondary icon-only-tiny">
                                    <Server size={12} />
                                </a>
                                <a href="#" onclick={(e) => { e.preventDefault(); e.stopPropagation(); appState.removeHost(host.id); }} class="secondary hover-error icon-only-tiny">
                                    <Trash2 size={12} />
                                </a>
                            </div>
                        </summary>
                        <ul>
                            {#each host.projects as project}
                                <li>
                                    <a href="#" onclick={(e) => { e.preventDefault(); appState.activateProject(host.id, project.id).then(() => { appState.triggerTerminalFocus(); appState.triggerTerminalResize(); }); }}>
                                        <Play size={10} /> {project.name || project.session_name}
                                    </a>
                                </li>
                            {/each}
                        </ul>
                    </details>
                </li>
            {/each}
        </ul>
    </div>

    <!-- Bottom Settings Button -->
    <div class="sidebar-footer">
        <div class="sidebar-actions grid">
            <button class="outline contrast action-btn" onclick={() => { showAddHost = !showAddHost; showAddSession = false; showSettings = false; }}>
                <Server size={14} />
                <span>Host</span>
            </button>
            <button class="outline contrast action-btn" onclick={() => { showAddSession = !showAddSession; showAddHost = false; showSettings = false; }}>
                <Terminal size={14} />
                <span>Session</span>
            </button>
        </div>
        <button class="outline contrast settings-trigger" onclick={() => { showSettings = !showSettings; showAddHost = false; showAddSession = false; }}>
            <Settings size={16} />
            <span>Settings</span>
        </button>
    </div>
</aside>

<style>
    .sidebar {
        position: absolute;
        top: 0;
        left: 0;
        bottom: 0;
        width: 250px;
        background-color: var(--bg-darker);
        border-right: 1px solid var(--pico-border-color);
        display: flex;
        flex-direction: column;
        z-index: 50;
        padding: 0;
        box-shadow: 4px 0 10px rgba(0, 0, 0, 0.3);
    }

    nav {
        padding: 0.5rem 1rem;
        border-bottom: 1px solid var(--pico-border-color);
        display: flex;
        justify-content: space-between;
        align-items: center;
        min-height: 3rem;
    }
    
    nav ul {
        margin: 0;
        padding: 0;
        display: flex;
        gap: 0.5rem;
    }
    
    nav li {
        list-style: none;
        padding: 0;
    }

    .sidebar-footer {
        padding: 0.75rem 1rem;
        border-top: 1px solid var(--pico-border-color);
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
    }

    .sidebar-actions {
        margin-bottom: 0;
    }

    .action-btn, .settings-trigger {
        width: 100%;
        display: flex;
        align-items: center;
        gap: 0.75rem;
        padding: 0.5rem 0.75rem;
        font-size: 0.85rem;
        margin-bottom: 0;
        border-width: 1px;
    }

    .action-btn {
        justify-content: center;
        padding: 0.4rem;
    }

    .icon-only {
        padding: 0.25rem;
        border: none;
        display: flex;
    }

    .icon-only-tiny {
        padding: 2px;
        display: flex;
        color: var(--pico-muted-color);
    }
    .icon-only-tiny:hover {
        color: var(--pico-primary);
    }
    .icon-only-tiny.hover-error:hover {
        color: var(--error);
    }

    .form-card {
        margin: 0;
        border-radius: 0;
        border-bottom: 1px solid var(--pico-border-color);
        background-color: var(--bg-input);
    }
    
    .form-card header {
        padding: 0.5rem 1rem;
        display: flex;
        justify-content: space-between;
        align-items: center;
        margin-bottom: 0.5rem;
    }
    
    .form-card form {
        padding: 0 1rem 1rem 1rem;
        margin-bottom: 0;
    }
    
    .form-card input {
        margin-bottom: 0.5rem;
        font-size: 0.8rem;
        padding: 0.4rem;
        height: auto;
    }
    
    .form-card button {
        font-size: 0.8rem;
        padding: 0.4rem;
        margin-bottom: 0;
    }

    .close {
        color: var(--pico-muted-color);
        text-decoration: none;
    }
    .close:hover {
        color: var(--error);
    }

    .tree-container {
        flex: 1;
        overflow-y: auto;
        padding: 1rem;
    }

    .tree-container ul {
        padding-left: 0;
        margin-bottom: 0.5rem;
    }
    
    .tree-container li {
        list-style: none;
        margin-bottom: 0.25rem;
    }

    .host-summary {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.25rem;
        font-size: 0.85rem;
        list-style: none;
        cursor: pointer;
    }
    
    .flex-row {
        display: flex;
        align-items: center;
        gap: 0.5rem;
    }

    .session-row-container {
        display: flex;
        gap: 0.25rem;
        align-items: stretch;
    }

    .delete-btn {
        width: auto;
        padding: 0 0.5rem;
        border-color: transparent;
        color: var(--pico-muted-color);
    }
    .delete-btn:hover {
        color: var(--error);
        background: transparent;
        border-color: var(--error);
    }

    a[role="button"] {
        width: 100%;
        text-align: left;
        padding: 0.4rem;
        font-size: 0.85rem;
        border-width: 1px;
    }
    
    small {
        font-size: 0.7rem;
        font-weight: bold;
        color: var(--pico-muted-color);
        letter-spacing: 0.05em;
    }

    .text-error { color: var(--error); }
</style>
