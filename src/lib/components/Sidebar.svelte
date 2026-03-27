<script lang="ts">
    import { appState } from '$lib/stores/app.svelte';
    import { Plus, Server, Terminal, AlertCircle, Globe, Play, Monitor, X, Trash2, Circle, Settings, Copy, Key, RotateCcw } from 'lucide-svelte';

    let showAddHost = $state(false);
    let showAddSession = $state(false);
    let showSettings = $state(false);
    let newKeyLabel = $state('');
    let showKeyForm = $state(false);
    
    // Host form
    let newHostAddress = $state('');
    let newHostUser = $state('');
    let newHostPass = $state('');
    let newHostAuthMethod = $state<'Password' | 'Key' | 'PrivateKey'>('Password');
    let newHostKeyId = $state('');
    let newHostPrivateKeyPath = $state('');

    // Session form
    let newSessionName = $state('');
    let newSessionHost = $state('');
    let newSessionUser = $state('');
    let newSessionPass = $state('');
    let newSessionAuthMethod = $state<'Password' | 'Key' | 'PrivateKey'>('Password');
    let newSessionKeyId = $state('');
    let newSessionPrivateKeyPath = $state('');

    async function handleAddHost() {
        if (newHostAddress && newHostUser) {
            await appState.addHost(
                newHostAddress,
                newHostAddress,
                newHostUser,
                newHostAuthMethod === 'Password' ? newHostPass : undefined,
                newHostAuthMethod === 'Key' ? newHostKeyId : undefined,
                newHostAuthMethod === 'PrivateKey' ? newHostPrivateKeyPath : undefined,
            );
            resetHostForm();
        }
    }

    function resetHostForm() {
        newHostAddress = ''; newHostUser = ''; newHostPass = '';
        newHostKeyId = ''; newHostPrivateKeyPath = ''; newHostAuthMethod = 'Password';
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
                newSessionAuthMethod === 'Key' ? newSessionKeyId : undefined,
                newSessionAuthMethod === 'PrivateKey' ? newSessionPrivateKeyPath : undefined
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
        newSessionPrivateKeyPath = ''; newSessionKeyId = '';
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
    <!-- Forms -->
    {#if showAddHost}
        <article class="form-card">
            <header>
                <strong>New Host</strong>
                <button type="button" class="close" onclick={resetHostForm}><X size={14} /></button>
            </header>
            <form onsubmit={(e) => { e.preventDefault(); handleAddHost(); }}>
                <input type="text" placeholder="Address (IP/FQDN)" bind:value={newHostAddress} aria-label="Address" required />
                <input type="text" placeholder="Username" bind:value={newHostUser} aria-label="Username" required />
                <select bind:value={newHostAuthMethod} aria-label="Host Auth Method">
                    <option value="Password">Password</option>
                    <option value="Key">SSH Identity</option>
                    <option value="PrivateKey">Private Key File</option>
                </select>
                {#if newHostAuthMethod === 'Password'}
                    <input type="password" placeholder="Password (Optional)" bind:value={newHostPass} aria-label="Password" />
                {:else if newHostAuthMethod === 'Key'}
                    <select bind:value={newHostKeyId} aria-label="Host SSH Identity" required>
                        <option value="" disabled selected>Select Identity...</option>
                        {#each appState.sshKeys as key}
                            <option value={key.id}>{key.label}</option>
                        {/each}
                    </select>
                {:else}
                    <input type="text" placeholder="~/.ssh/id_ed25519" bind:value={newHostPrivateKeyPath} aria-label="Host Private Key Path" required />
                {/if}
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
                <button type="button" class="close" onclick={resetSessionForm}><X size={14} /></button>
            </header>
            <form onsubmit={(e) => { e.preventDefault(); handleAddSession(); }}>
                <input type="text" placeholder="Name" bind:value={newSessionName} aria-label="Name" required />
                <input type="text" placeholder="Host Address" bind:value={newSessionHost} aria-label="Host Address" required />
                <input type="text" placeholder="Username" bind:value={newSessionUser} aria-label="Username" required />
                
                <select bind:value={newSessionAuthMethod} aria-label="Auth Method">
                    <option value="Password">Password</option>
                    <option value="Key">SSH Identity</option>
                    <option value="PrivateKey">Private Key File</option>
                </select>

                {#if newSessionAuthMethod === 'Password'}
                    <input type="password" placeholder="Password" bind:value={newSessionPass} aria-label="Password" />
                {:else if newSessionAuthMethod === 'Key'}
                    <select bind:value={newSessionKeyId} aria-label="SSH Identity" required>
                        <option value="" disabled selected>Select Identity...</option>
                        {#each appState.sshKeys as key}
                            <option value={key.id}>{key.label}</option>
                        {/each}
                    </select>
                {:else if newSessionAuthMethod === 'PrivateKey'}
                    <input type="text" placeholder="~/.ssh/id_ed25519" bind:value={newSessionPrivateKeyPath} aria-label="Private Key Path" required />
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
                <button type="button" class="close" onclick={() => showSettings = false}><X size={14} /></button>
            </header>
            <div class="settings-content">
                <div class="settings-item">
                    <p class="settings-label-main">TERMINAL FONT</p>
                    <div class="grid">
                        <label>
                            Size (px)
                            <input 
                                type="number" 
                                value={appState.terminalFontSize} 
                                onchange={(e) => appState.setTerminalFontSize(parseInt(e.currentTarget.value) || 14)}
                            />
                        </label>
                        <label>
                            Weight
                            <select 
                                value={appState.terminalFontWeight} 
                                onchange={(e) => appState.setTerminalFontWeight(e.currentTarget.value)}
                            >
                                <option value="normal">Normal</option>
                                <option value="bold">Bold</option>
                                <option value="100">100</option>
                                <option value="200">200</option>
                                <option value="300">300</option>
                                <option value="400">400</option>
                                <option value="500">500</option>
                                <option value="600">600</option>
                                <option value="700">700</option>
                                <option value="800">800</option>
                                <option value="900">900</option>
                            </select>
                        </label>
                    </div>
                </div>

                <div class="settings-item">
                    <p class="settings-label-main">DOCUMENT FONT</p>
                    <div class="grid">
                        <label>
                            Size (px)
                            <input 
                                type="number" 
                                value={appState.markdownFontSize} 
                                onchange={(e) => appState.setMarkdownFontSize(parseInt(e.currentTarget.value) || 16)}
                            />
                        </label>
                        <label>
                            Weight
                            <select 
                                value={appState.markdownFontWeight} 
                                onchange={(e) => appState.setMarkdownFontWeight(e.currentTarget.value)}
                            >
                                <option value="100">100</option>
                                <option value="200">200</option>
                                <option value="300">300</option>
                                <option value="400">400</option>
                                <option value="500">500</option>
                                <option value="600">600</option>
                                <option value="700">700</option>
                                <option value="800">800</option>
                                <option value="900">900</option>
                            </select>
                        </label>
                    </div>
                </div>

                <div class="settings-item">
                    <button class="btn-sm" onclick={() => { showSettings = false; }}>Save Settings</button>
                </div>

                <div class="ssh-keys-section settings-item">
                    <div class="flex-row between mb-2">
                        <small>SSH IDENTITIES</small>
                        <button class="outline contrast icon-only-tiny" onclick={() => { showKeyForm = !showKeyForm; }} title="Generate New Key">
                            <Plus size={14} />
                        </button>
                    </div>

                    {#if showKeyForm}
                        <form class="key-gen-form" onsubmit={(e) => { e.preventDefault(); if (newKeyLabel.trim()) { appState.generateSshKey(newKeyLabel.trim()); newKeyLabel = ''; showKeyForm = false; } }}>
                            <div class="flex-row">
                                <input type="text" placeholder="Key label" bind:value={newKeyLabel} aria-label="Key Label" required />
                                <button type="submit">
                                    <Key size={12} />
                                </button>
                            </div>
                        </form>
                    {/if}

                    {#if appState.sshKeys.length === 0}
                        <p class="text-xs secondary">No keys generated yet.</p>
                    {:else}
                        <ul class="list-none p-0">
                            {#each appState.sshKeys as key}
                                <li class="key-item mb-2">
                                    <div class="flex-row between">
                                        <span class="text-sm"><Key size={10} /> {key.label}</span>
                                        <div class="flex-row">
                                            <button class="secondary icon-only-tiny" onclick={() => { navigator.clipboard.writeText('ssh-ed25519 ' + key.public_key); }} title="Copy Public Key">
                                                <Copy size={10} />
                                            </button>
                                            <button class="secondary hover-error icon-only-tiny" onclick={() => appState.deleteSshKey(key.id)} title="Delete Key">
                                                <Trash2 size={10} />
                                            </button>
                                        </div>
                                    </div>
                                    <div class="key-public-preview">ssh-ed25519 {key.public_key}</div>
                                </li>
                            {/each}
                        </ul>
                    {/if}
                </div>
            </div>
        </article>
    {/if}

    <div class="tree-container scrollbar-hide">
        <!-- Project Files: shown when a project is active -->
        {#if appState.activeProject && (appState.projectMdFiles.root.length > 0 || appState.projectMdFiles.docs.length > 0)}
            {@const proj = appState.activeProject}
            <small class="mt-0 block">PROJECT FILES</small>
            <ul class="list-none">
                {#each appState.projectMdFiles.root as file}
                    <li>
                        <button
                            type="button"
                            class="project-btn"
                            class:active-file={appState.openMarkdownFiles.includes(`${proj.id}/${file}`)}
                            onclick={() => appState.openMarkdownFile(`${proj.id}/${file}`)}
                        >
                            <span class="file-icon">📄</span> {file}
                        </button>
                    </li>
                {/each}
                {#if appState.projectMdFiles.docs.length > 0}
                    <li>
                        <details>
                            <summary class="host-summary">
                                <div class="flex-row"><span class="file-icon">📁</span> docs</div>
                            </summary>
                            <ul>
                                {#each appState.projectMdFiles.docs as file}
                                    <li>
                                        <button
                                            type="button"
                                            class="project-btn"
                                            class:active-file={appState.openMarkdownFiles.includes(`${proj.id}/${file}`)}
                                            onclick={() => appState.openMarkdownFile(`${proj.id}/${file}`)}
                                        >
                                            {file.replace('docs/', '')}
                                        </button>
                                    </li>
                                {/each}
                            </ul>
                        </details>
                    </li>
                {/if}
            </ul>
        {/if}

        <!-- Sessions Section -->
        <small>SESSIONS</small>
        <ul class="list-none">
            {#each appState.sessions as session}
                <li>
                    <div class="session-row-container">
                        <button
                           class={appState.activeSessionId === session.id ? 'session-btn primary' : 'session-btn secondary outline'}
                           onclick={() => { appState.connectSession(session.id); appState.triggerTerminalFocus(); appState.triggerTerminalResize(); }}
                           style="flex: 1;"
                        >
                            <div class="flex-row">
                                <Circle size={8} fill={getSessionStatusColor(session.status)} stroke="none" />
                                <Monitor size={14} /> {session.label}
                            </div>
                        </button>
                        <button class="outline contrast delete-btn" onclick={() => appState.removeSession(session.id)} aria-label="Delete Session">
                            <Trash2 size={14} />
                        </button>
                    </div>
                </li>
            {/each}
        </ul>

        <!-- Hosts Section -->
        <small>HOSTS</small>
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
                            <div class="flex-row">
                                <button type="button" onclick={(e) => { e.stopPropagation(); appState.fetchProjectsForHost(host.id); }} class="secondary icon-only-tiny">
                                    <Server size={12} />
                                </button>
                                <button type="button" onclick={(e) => { e.stopPropagation(); appState.removeHost(host.id); }} class="secondary hover-error icon-only-tiny">
                                    <Trash2 size={12} />
                                </button>
                            </div>
                        </summary>
                        <ul>
                            {#each host.projects as project}
                                <li>
                                    <button type="button" class="project-btn" onclick={() => { appState.activateProject(host.id, project.id).then(() => { appState.triggerTerminalFocus(); appState.triggerTerminalResize(); }); }}>
                                        <Play size={10} /> {project.name || project.session_name}
                                    </button>
                                </li>
                            {/each}
                        </ul>
                    </details>
                </li>
            {/each}
        </ul>
    </div>

    <!-- Footer: icon-only buttons in a single row -->
    <div class="sidebar-footer">
        <div class="footer-row">
            {#if appState.activeSessionId}
                <button class="outline icon-btn" title="Restart Terminal" onclick={() => appState.restartActiveSession()}>
                    <RotateCcw size={16} />
                </button>
            {/if}
            <button class="outline contrast icon-btn" title="Add Host" onclick={() => { showAddHost = !showAddHost; showAddSession = false; showSettings = false; }}>
                <Server size={16} />
            </button>
            <button class="outline contrast icon-btn" title="Add Session" onclick={() => { showAddSession = !showAddSession; showAddHost = false; showSettings = false; }}>
                <Terminal size={16} />
            </button>
            <button class="outline contrast icon-btn" title="Settings" onclick={() => { showSettings = !showSettings; showAddHost = false; showAddSession = false; }}>
                <Settings size={16} />
            </button>
        </div>
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
        overflow: hidden;
    }

    .sidebar-footer {
        padding: 0.5rem 0.75rem;
        border-top: 1px solid var(--pico-border-color);
        flex-shrink: 0;
    }

    .footer-row {
        display: flex;
        gap: 0.3rem;
        align-items: center;
    }

    .icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0.4rem;
        margin: 0;
        flex: 1;
        min-width: 0;
        border-width: 1px;
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

    .settings-content {
        padding: 0 1.25rem 1.25rem 1.25rem;
    }

    .settings-item {
        margin-bottom: 1.5rem;
    }

    .settings-label-main {
        font-size: 0.7rem;
        font-weight: bold;
        color: var(--fg-dim);
        letter-spacing: 0.05em;
        margin-bottom: 0.5rem;
        display: block;
    }

    .settings-item input {
        font-size: 0.85rem;
        padding: 0.4rem;
        margin-bottom: 0;
    }

    .settings-item label {
        font-size: 0.75rem;
        margin-bottom: 0.25rem;
    }

    .tree-container {
        flex: 1;
        overflow-y: auto;
        min-height: 0;
        padding: 0.4rem 0.6rem;
    }

    .tree-container ul {
        padding-left: 0;
        margin-bottom: 0.15rem;
    }

    .tree-container li {
        list-style: none;
        margin-bottom: 0;
    }

    .host-summary {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 0.1rem 0.15rem;
        font-size: 0.85rem;
        list-style: none;
        cursor: pointer;
    }

    .session-row-container {
        display: flex;
        gap: 0.15rem;
        align-items: stretch;
    }

    .delete-btn {
        width: auto;
        padding: 0 0.4rem;
        border-color: transparent;
        color: var(--pico-muted-color);
    }
    .delete-btn:hover {
        color: var(--error);
        background: transparent;
        border-color: var(--error);
    }

    button.close {
        background: none;
        border: none;
        padding: 0;
        display: flex;
        cursor: pointer;
        margin: 0;
    }

    .session-btn {
        width: 100%;
        text-align: left;
        padding: 0.2rem 0.4rem;
        font-size: 0.85rem;
        border-width: 1px;
    }

    .project-btn {
        background: none;
        border: none;
        padding: 0.02rem 0.06rem;
        font-size: 0.85rem;
        cursor: pointer;
        display: flex;
        align-items: center;
        gap: 0.25rem;
        color: var(--pico-color);
        width: 100%;
        text-align: left;
        margin: 0;
    }
    .project-btn:hover {
        color: var(--pico-primary);
    }

    small {
        font-size: 0.7rem;
        font-weight: bold;
        color: var(--pico-muted-color);
        letter-spacing: 0.05em;
        display: block;
        margin-bottom: 0.1rem;
    }

    .key-gen-form input {
        margin-bottom: 0;
        font-size: 0.8rem;
        padding: 0.3rem;
    }
    .key-gen-form button {
        margin-bottom: 0;
        padding: 0.3rem 0.5rem;
        font-size: 0.8rem;
        white-space: nowrap;
    }

    .key-public-preview {
        font-size: 0.7rem;
        color: var(--fg-dim);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        max-width: 180px;
    }

    .text-error { color: var(--error); }

    .active-file {
        color: var(--pico-primary);
        font-weight: 600;
    }
    .file-icon {
        font-size: 0.75rem;
    }
</style>
