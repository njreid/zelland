import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { load } from "@tauri-apps/plugin-store";
import { insertOrdered } from '$lib/utils/md-ordering';

export interface Project {
    id: string;
    name?: string;
    host: string;
    session_name: string;
    root_path: string;
    hostId: string;
}

export interface Host {
    id: string;
    label: string;
    address: string;
    username: string;
    port: number;
    password?: string;
    key_id?: string;
    private_key_path?: string;
    reachable: boolean;
    error?: string;
    helperError?: string;
    projects: Project[];
}

export interface DaemonRecentSession {
    sessionName: string;
    hostAddress: string;
    hostLabel: string;
    connectedAt: string;
    readmeExcerpt?: string;
}

export interface AgentNotification {
    title: string;
    body: string;
    question: string;
    source: number;  // NotificationSource int: 0=User, 1=ClaudeCode, 2=GeminiCli, 3=ZellijPlugin
    session_name: string | null;
    tab_index: number;
    pane_id: number;
}

export interface Session {
    id: string;
    label: string;
    hostAddress: string;
    username: string;
    password?: string;
    port: number;
    type: 'ssh';
    zellijSession: string;
    projectRoot?: string;
    key_id?: string;
    private_key_path?: string;
    status: 'disconnected' | 'connecting' | 'connected' | 'error';
}

const STORE_PATH = "settings.json";

function createAppState() {
    // --- State ---
    let hosts = $state<Host[]>([]);
    let sessions = $state<Session[]>([]);
    let activeSessionId = $state<string | null>(null);
    let terminalFocusTrigger = $state(0);
    let terminalResizeTrigger = $state(0);
    let viewUpdateTrigger = $state<Record<string, number>>({});
    let loadedFiles = $state<Record<string, boolean>>({});
    let openMarkdownFiles = $state<string[]>([]);
    let projectMdFiles = $state<{ root: string[]; docs: string[] }>({ root: [], docs: [] });
    let navigationTrigger = $state(-1);
    let recentSessionIds = $state<string[]>([]);
    let terminalFontSize = $state(14);
    let terminalFontWeight = $state('400');
    let markdownFontSize = $state(16);
    let markdownFontWeight = $state('400');
    let logs = $state<{ message: string, type: 'info' | 'error' }[]>([]);
    let sshKeys = $state<any[]>([]);
    let daemonConnected = $state(false);
    let daemonRecentSessions = $state<DaemonRecentSession[]>([]);
    let lastAgentNotification = $state<AgentNotification | null>(null);
    let pinnedProjects = $state<{hostId: string, projectName: string}[]>([]);

    // --- Derived ---
    const activeSession = $derived(
        activeSessionId ? sessions.find(s => s.id === activeSessionId) || null : null
    );

    const activeProject = $derived.by(() => {
        const session = activeSession;
        if (!session) return null;
        const host = hosts.find(h => h.address === session.hostAddress);
        return host?.projects.find(p => p.session_name === session.zellijSession) || null;
    });

    const recentSessions = $derived(
        recentSessionIds
            .map(id => sessions.find(s => s.id === id))
            .filter((s): s is Session => s !== undefined)
    );

    // --- MD file ordering (logic in src/lib/utils/md-ordering.ts) ---
    const PRIORITY_ORDER = ['README.md', 'DESIGN.md', 'PLAN.md'];

    async function loadProjectFiles(hostAddress: string, projectId: string) {
        try {
            const files = await invoke<{ root_md: string[]; docs_md: string[] }>(
                'daemon_list_project_files',
                { url: `http://${hostAddress}:8083`, projectId }
            );
            projectMdFiles = { root: files.root_md, docs: files.docs_md };
            // Auto-open priority files that exist in the project
            for (const name of PRIORITY_ORDER) {
                if (files.root_md.includes(name)) {
                    const fullPath = `${projectId}/${name}`;
                    openMarkdownFiles = insertOrdered(openMarkdownFiles, fullPath);
                }
            }
        } catch (e) {
            console.warn('Failed to load project files:', e);
        }
    }

    // --- Helpers ---
    function log(message: string, type: 'info' | 'error' = 'info') {
        logs.push({ message, type });
        if (logs.length > 50) logs.shift();
    }

    function buildSshConfig(target: {
        host: string;
        port?: number;
        username: string;
        password?: string;
        key_id?: string;
        private_key_path?: string;
        session_name: string;
        project_root?: string;
    }) {
        const authMethod = target.key_id ? "Key" : target.private_key_path ? "PrivateKey" : "Password";
        return {
            host: target.host,
            port: target.port ?? 22,
            username: target.username,
            auth_method: authMethod,
            password: target.password || null,
            private_key_path: target.private_key_path || null,
            private_key_passphrase: null,
            key_id: target.key_id || null,
            session_name: target.session_name,
            project_root: target.project_root || null,
        };
    }

    async function ensureSessionHelper(session: Session) {
        await invoke("ensure_remote_helper", {
            config: buildSshConfig({
                host: session.hostAddress,
                port: session.port,
                username: session.username,
                password: session.password,
                key_id: session.key_id,
                private_key_path: session.private_key_path,
                session_name: session.zellijSession,
                project_root: session.projectRoot,
            })
        });
    }

    async function ensureHostHelper(host: Host) {
        await invoke("ensure_remote_helper", {
            config: buildSshConfig({
                host: host.address,
                port: host.port,
                username: host.username,
                password: host.password,
                key_id: host.key_id,
                private_key_path: host.private_key_path,
                session_name: 'zelland-bootstrap',
            })
        });
    }

    async function fetchDaemonRecentSessions() {
        const results: DaemonRecentSession[] = [];
        for (const host of hosts) {
            try {
                const entries = await invoke<any[]>("daemon_get_recent_sessions", {
                    url: `http://${host.address}:8083`
                });
                for (const e of entries) {
                    results.push({
                        sessionName: e.session_name,
                        hostAddress: host.address,
                        hostLabel: host.label,
                        connectedAt: e.connected_at,
                        readmeExcerpt: e.readme_excerpt,
                    });
                }
            } catch {
                // Host unreachable — skip silently
            }
        }
        results.sort((a, b) => b.connectedAt.localeCompare(a.connectedAt));
        daemonRecentSessions = results.slice(0, 6);
    }

    async function saveToStore() {
        try {
            const store = await load(STORE_PATH);
            await store.set("hosts", hosts.map(({ id, label, address, username, port, password, key_id, private_key_path }) => ({
                id, label, address, username, port, password, key_id, private_key_path
            })));
            await store.set("sessions", sessions.map(({ id, label, hostAddress, username, password, port, type, zellijSession, projectRoot, key_id, private_key_path }) => ({
                id, label, hostAddress, username, password, port, type, zellijSession, projectRoot, key_id, private_key_path
            })));
            await store.set("terminalFontSize", terminalFontSize);
            await store.set("terminalFontWeight", terminalFontWeight);
            await store.set("markdownFontSize", markdownFontSize);
            await store.set("markdownFontWeight", markdownFontWeight);
            await store.set("recentSessionIds", recentSessionIds);
            await store.set("pinnedProjects", pinnedProjects.map(p => ({ ...p })));
            await store.save();
        } catch (e) {
            console.error("Failed to save to store:", e);
        }
    }

    async function fetchSshKeys() {
        try {
            sshKeys = await invoke<any[]>("list_ssh_keys");
        } catch (e) {
            console.error("Failed to fetch SSH keys:", e);
        }
    }

    async function fetchProjectsForHost(hostId: string) {
        const host = hosts.find(h => h.id === hostId);
        if (!host) return;

        log(`Fetching projects for ${host.label}...`);

        // Helper bootstrap is best-effort — a failure here doesn't block project fetch in
        // case the daemon was already running from a previous connection.
        try {
            await ensureHostHelper(host);
            host.helperError = undefined;
        } catch (e) {
            const err = String(e);
            console.warn(`Helper unavailable for ${host.label}:`, e);
            host.helperError = err;
            log(`Helper unavailable for ${host.label}: ${err}`, 'error');
        }

        try {
            const projects = await invoke<any[]>("daemon_get_projects", { url: `http://${host.address}:8083` });
            host.projects = projects.map(p => ({ ...p, hostId: host.id }));
            host.reachable = true;
            host.error = undefined;
            log(`Fetched ${projects.length} projects for ${host.label}.`);
        } catch (e) {
            const err = String(e);
            console.error(`Failed to fetch projects for host ${host.label}:`, e);
            host.reachable = false;
            host.error = host.helperError ? `Helper: ${host.helperError}` : err;
            log(`Failed to fetch projects for ${host.label}: ${err}`, 'error');
        }
    }

    // Define public methods using local variables
    const methods = {
        async fetchProjectsForHost(hostId: string) { await fetchProjectsForHost(hostId); },

        connectSession(sessionId: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (!session) return;
            session.status = 'connecting';
            activeSessionId = sessionId;
            log(`Connecting to session: ${session.label}...`);
        },

        triggerTerminalResize() { terminalResizeTrigger += 1; },
        
        scrollToPane(index: number) { navigationTrigger = index; },

        async runZellijAction(sessionId: string, action: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (!session) return;

            if (daemonConnected) {
                try {
                    await invoke("daemon_run_zellij_action", { action, sessionName: session.zellijSession });
                    return;
                } catch (e) {
                    console.warn("Failed to run zellij action via daemon, falling back to SSH:", e);
                }
            }

            const command = `zellij -s ${session.zellijSession} action ${action}`;
            try {
                await invoke("run_remote_command", {
                    config: buildSshConfig({
                        host: session.hostAddress,
                        port: session.port,
                        username: session.username,
                        password: session.password,
                        key_id: session.key_id,
                        private_key_path: session.private_key_path,
                        session_name: session.zellijSession,
                        project_root: session.projectRoot,
                    }),
                    command
                });
            } catch (e) {
                log(`Failed to run zellij action: ${e}`, 'error');
            }
        },

        async navigateToSession(zellijSessionName: string, tabIndex: number) {
            const session = sessions.find(s => s.zellijSession === zellijSessionName);
            if (!session) return;
            if (session.status !== 'connected') {
                this.connectSession(session.id);
            } else {
                activeSessionId = session.id;
                if (tabIndex > 0) {
                    await this.runZellijAction(session.id, `go-to-tab ${tabIndex}`);
                }
            }
        },

        dismissAgentNotification() { lastAgentNotification = null; },

        openMarkdownFile(filename: string) {
            const cleanName = filename.startsWith('./') ? filename.slice(2) : filename;
            openMarkdownFiles = insertOrdered(openMarkdownFiles, cleanName);
            this.scrollToPane(openMarkdownFiles.indexOf(cleanName) + 1);
        },

        triggerTerminalFocus() { terminalFocusTrigger += 1; },
        setFileLoaded(filename: string, loaded: boolean) { loadedFiles[filename] = loaded; },

        setTerminalFontSize(size: number) {
            terminalFontSize = size;
            saveToStore();
            this.triggerTerminalResize();
        },

        setTerminalFontWeight(weight: string) {
            terminalFontWeight = weight;
            saveToStore();
            this.triggerTerminalResize();
        },

        setMarkdownFontSize(size: number) {
            markdownFontSize = size;
            saveToStore();
        },

        setMarkdownFontWeight(weight: string) {
            markdownFontWeight = weight;
            saveToStore();
        },

        async generateSshKey(label: string) {
            try {
                await invoke("generate_ssh_key", { label });
                fetchSshKeys();
            } catch (e) {
                log(`Failed to generate SSH key: ${e}`, 'error');
            }
        },

        async deleteSshKey(id: string) {
            try {
                await invoke("delete_ssh_key", { id });
                fetchSshKeys();
            } catch (e) {
                log(`Failed to delete SSH key: ${e}`, 'error');
            }
        },

        async addHost(label: string, address: string, username: string, password?: string, keyId?: string, privateKeyPath?: string) {
            const id = crypto.randomUUID();
            hosts.push({
                id,
                label,
                address,
                username,
                port: 22,
                password,
                key_id: keyId,
                private_key_path: privateKeyPath,
                reachable: false,
                projects: []
            });
            await saveToStore();
            fetchProjectsForHost(id);
        },

        async fetchAllProjects() {
            for (const host of hosts) await fetchProjectsForHost(host.id);
        },

        async removeHost(hostId: string) {
            hosts = hosts.filter(h => h.id !== hostId);
            pinnedProjects = pinnedProjects.filter(p => p.hostId !== hostId);
            await saveToStore();
        },

        async addSession(label: string, hostAddress: string, username: string, type: 'ssh', zellijSession: string, password?: string, keyId?: string, privateKeyPath?: string) {
            const id = crypto.randomUUID();
            const host = hosts.find(h => h.address === hostAddress && h.username === username);
            const project = host?.projects.find(p => p.session_name === zellijSession);
            sessions.push({
                id, label, hostAddress, username, password,
                port: host?.port ?? 22, type, zellijSession, status: 'disconnected',
                projectRoot: project?.root_path,
                key_id: keyId, private_key_path: privateKeyPath
            });
            if (host) {
                host.password = password;
                host.key_id = keyId;
                host.private_key_path = privateKeyPath;
            }
            await saveToStore();
        },

        async removeSession(sessionId: string) {
            sessions = sessions.filter(s => s.id !== sessionId);
            if (activeSessionId === sessionId) activeSessionId = null;
            await saveToStore();
        },

        togglePinProject(hostId: string, projectName: string) {
            const idx = pinnedProjects.findIndex(p => p.hostId === hostId && p.projectName === projectName);
            if (idx >= 0) {
                pinnedProjects = pinnedProjects.filter((_, i) => i !== idx);
            } else {
                pinnedProjects = [...pinnedProjects, { hostId, projectName }];
            }
            saveToStore();
        },

        unpinProject(hostId: string, projectName: string) {
            pinnedProjects = pinnedProjects.filter(p => !(p.hostId === hostId && p.projectName === projectName));
            saveToStore();
        },

        async openProjectByName(hostId: string, projectName: string) {
            const host = hosts.find(h => h.id === hostId);
            if (!host) return;
            const project = host.projects.find(p => (p.name ?? p.session_name) === projectName);
            if (project) {
                await this.activateProject(hostId, project.id);
                return;
            }
            // Project not yet in cached list — create session using project name
            const sessionId = `project-${hostId}-${projectName}`;
            let session = sessions.find(s => s.id === sessionId);
            if (!session) {
                session = {
                    id: sessionId, label: projectName,
                    hostAddress: host.address, username: host.username, password: host.password,
                    port: host.port, type: 'ssh', zellijSession: projectName,
                    projectRoot: `~/code/${projectName}`,
                    key_id: host.key_id, private_key_path: host.private_key_path,
                    status: 'disconnected'
                };
                sessions.push(session);
                await saveToStore();
            }
            this.connectSession(sessionId);
        },

        getSshConfig(sessionId: string) {
            const session = sessions.find(s => s.id === sessionId);
            return session ? buildSshConfig({
                host: session.hostAddress,
                port: session.port,
                username: session.username,
                password: session.password,
                key_id: session.key_id,
                private_key_path: session.private_key_path,
                session_name: session.zellijSession,
                project_root: session.projectRoot,
            }) : null;
        },

        onSessionConnected(sessionId: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (!session) return;
            session.status = 'connected';
            log(`Connected to session: ${session.label}.`);
            recentSessionIds = [sessionId, ...recentSessionIds.filter(id => id !== sessionId)].slice(0, 3);
            saveToStore();

            (async () => {
                try {
                    await ensureSessionHelper(session);
                    await invoke("daemon_record_session", {
                        url: `http://${session.hostAddress}:8083`,
                        sessionName: session.zellijSession
                    });
                    fetchDaemonRecentSessions().catch(() => {});
                    await invoke("daemon_connect", { url: `ws://${session.hostAddress}:8083/ws` });
                    daemonConnected = true;
                } catch (e) {
                    const err = String(e);
                    console.warn("Failed to bootstrap remote helper:", e);
                    log(`Helper unavailable for this session: ${err}`, 'error');
                    daemonConnected = false;
                }
            })();

            this.triggerTerminalResize();

            // Fetch .md files for the newly connected project
            const connectedSession = sessions.find(s => s.id === sessionId);
            if (connectedSession) {
                const host = hosts.find(h => h.address === connectedSession.hostAddress);
                const project = host?.projects.find(p => p.session_name === connectedSession.zellijSession);
                if (host && project) {
                    loadProjectFiles(host.address, project.id);
                }
            }
        },

        onSessionConnectionFailed(sessionId: string, error: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (session) session.status = 'error';
            daemonConnected = false;
            log(`Connection failed for ${session?.label}: ${error}`, 'error');
        },

        onSessionClosed(sessionId: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (session) session.status = 'disconnected';
            if (activeSessionId === sessionId) {
                activeSessionId = null;
                projectMdFiles = { root: [], docs: [] };
                openMarkdownFiles = [];
            }
        },

        async activateProject(hostId: string, projectId: string) {
            const host = hosts.find(h => h.id === hostId);
            if (!host) return;
            const project = host.projects.find(p => p.id === projectId);
            if (!project) return;

            const sessionId = `project-${project.id}`;
            let session = sessions.find(s => s.id === sessionId);
            
            if (!session) {
                session = {
                    id: sessionId,
                    label: project.name || project.session_name,
                    hostAddress: host.address,
                    username: host.username,
                    password: host.password,
                    port: host.port,
                    type: 'ssh',
                    zellijSession: project.session_name,
                    projectRoot: project.root_path,
                    key_id: host.key_id,
                    private_key_path: host.private_key_path,
                    status: 'disconnected'
                };
                sessions.push(session);
                await saveToStore();
            } else {
                session.projectRoot = project.root_path;
                session.password = host.password;
                session.key_id = host.key_id;
                session.private_key_path = host.private_key_path;
                await saveToStore();
            }

            this.connectSession(sessionId);
        },

        async connectDaemonSession(entry: DaemonRecentSession) {
            const host = hosts.find(h => h.address === entry.hostAddress);
            if (!host) return;
            const project = host.projects.find(p => p.session_name === entry.sessionName);

            const sessionId = `daemon-${entry.hostAddress}-${entry.sessionName}`;
            let session = sessions.find(
                s => s.id === sessionId ||
                    (s.hostAddress === entry.hostAddress && s.zellijSession === entry.sessionName)
            );

            if (!session) {
                session = {
                    id: sessionId,
                    label: entry.sessionName,
                    hostAddress: host.address,
                    username: host.username,
                    password: host.password,
                    port: host.port,
                    type: 'ssh',
                    zellijSession: entry.sessionName,
                    projectRoot: project?.root_path,
                    key_id: host.key_id,
                    private_key_path: host.private_key_path,
                    status: 'disconnected'
                };
                sessions.push(session);
                await saveToStore();
            } else {
                session.projectRoot = project?.root_path ?? session.projectRoot;
                session.password = host.password;
                session.key_id = host.key_id;
                session.private_key_path = host.private_key_path;
                await saveToStore();
            }

            this.connectSession(session.id);
        },

        async closeActiveSession() {
            if (!activeSessionId) return;
            try {
                await invoke("ssh_disconnect", { tabId: activeSessionId });
            } catch (e) {
                console.warn("SSH disconnect failed:", e);
            }
            const session = sessions.find(s => s.id === activeSessionId);
            if (session) session.status = 'disconnected';
            activeSessionId = null;
        },

        async restartActiveSession() {
            if (!activeSessionId) return;
            const sessionId = activeSessionId;
            const session = sessions.find(s => s.id === sessionId);
            if (!session) return;

            try { await invoke("ssh_disconnect", { tabId: sessionId }); } catch {}
            session.status = 'disconnected';
            activeSessionId = null;

            await new Promise(resolve => setTimeout(resolve, 50));
            this.connectSession(sessionId);
        },

        writeInput(sessionId: string, data: Uint8Array | number[]) {
            invoke("ssh_write", { tabId: sessionId, data }).catch(() => {});
        },

        scrollTerminal(sessionId: string, delta: number) {
            invoke("ssh_scroll", { tabId: sessionId, delta }).catch(() => {});
        },

        async resize(sessionId: string, rows: number, cols: number) {
            try {
                await invoke("ssh_resize", { tabId: sessionId, rows, cols });
            } catch (e) { }
        }
    };

    // --- Initialization function ---
    async function init() {
        try {
            const store = await load(STORE_PATH);
            const savedHosts = await store.get<Host[]>("hosts");
            const savedSessions = await store.get<Session[]>("sessions");
            const savedFontSize = await store.get<number>("terminalFontSize");
            const savedFontWeight = await store.get<string>("terminalFontWeight");
            const savedMarkdownFontSize = await store.get<number>("markdownFontSize");
            const savedMarkdownFontWeight = await store.get<string>("markdownFontWeight");
            const savedRecentIds = await store.get<string[]>("recentSessionIds");
            const savedPinnedProjects = await store.get<{hostId: string, projectName: string}[]>("pinnedProjects");

            if (savedHosts) hosts = savedHosts.map(h => ({ ...h, port: h.port ?? 22, reachable: false, projects: [] }));
            if (savedSessions) sessions = savedSessions.map(s => ({ ...s, port: s.port ?? 22, status: 'disconnected' }));
            if (savedFontSize) terminalFontSize = savedFontSize;
            if (savedFontWeight) terminalFontWeight = savedFontWeight;
            if (savedMarkdownFontSize) markdownFontSize = savedMarkdownFontSize;
            if (savedMarkdownFontWeight) markdownFontWeight = savedMarkdownFontWeight;
            if (savedRecentIds) recentSessionIds = savedRecentIds;
            if (savedPinnedProjects) pinnedProjects = savedPinnedProjects;
            
            for (const host of hosts) fetchProjectsForHost(host.id);
            fetchSshKeys();
            fetchDaemonRecentSessions();
        } catch (e) {
            console.error("Failed to load store:", e);
        }

        listen("daemon-event", (event: any) => {
            const payload = event.payload.payload;
            if (payload?.OpenView) {
                const req = payload.OpenView;
                log(`Received view update: ${req.title}`);
                if (!viewUpdateTrigger[req.title]) viewUpdateTrigger[req.title] = 0;
                viewUpdateTrigger[req.title] += 1;
                // If a new .md file appeared, add it to projectMdFiles if it belongs to the active project
                const title = req.title as string;
                if (title.endsWith('.md') && activeProject) {
                    if (!projectMdFiles.root.includes(title) && !projectMdFiles.docs.includes(title)) {
                        projectMdFiles = {
                            ...projectMdFiles,
                            root: [...projectMdFiles.root, title].sort()
                        };
                    }
                }
            }
        });

        listen("tunnel-status", (event) => log(`Tunnel status: ${event.payload}`));
        listen("tunnel-error", (event) => log(`Tunnel error: ${event.payload}`, 'error'));

        listen("biometric-request", async (event: any) => {
            const request = event.payload;
            log(`Biometric auth requested for key: ${request.key_id}`);
            try {
                await invoke("biometric_result", {
                    response: { request_id: request.request_id, success: true, error: null }
                });
            } catch (e) {
                console.error("Failed to handle biometric request:", e);
                await invoke("biometric_result", {
                    response: { request_id: request.request_id, success: false, error: String(e) }
                });
            }
        });

        listen<AgentNotification>("agent-notification", ({ payload }) => {
            lastAgentNotification = payload;
            log(`Agent notification: ${payload.title}`);
            // When the app is backgrounded, request a rich Android notification with a
            // "Switch to <session>" action button.
            if (document.hidden && payload.session_name) {
                invoke('show_agent_notification', {
                    title: payload.title,
                    body: payload.question || payload.body,
                    sessionName: payload.session_name,
                }).catch(() => {});
            }
        });

        listen<any>("navigate-to-session", (event) => {
            const payload = event.payload;
            let session_name: string | null = null;
            let tab_index = 0;

            if (typeof payload === 'string') {
                try {
                    const parsed = JSON.parse(payload);
                    session_name = parsed.session_name;
                    tab_index = parsed.tab_index ?? 0;
                } catch {
                    session_name = payload;
                }
            } else if (payload && typeof payload === 'object') {
                session_name = payload.session_name;
                tab_index = payload.tab_index ?? 0;
            }

            if (session_name) {
                const session = sessions.find(s => s.zellijSession === session_name);
                if (session) {
                    if (session.status !== 'connected') {
                        methods.connectSession(session.id);
                    } else {
                        activeSessionId = session.id;
                        if (tab_index > 0) {
                            setTimeout(() => {
                                const command = `zellij -s ${session.zellijSession} action go-to-tab ${tab_index}`;
                                invoke("run_remote_command", {
                                    config: buildSshConfig({
                                        host: session.hostAddress,
                                        port: session.port,
                                        username: session.username,
                                        password: session.password,
                                        key_id: session.key_id,
                                        private_key_path: session.private_key_path,
                                        session_name: session.zellijSession,
                                        project_root: session.projectRoot,
                                    }),
                                    command
                                }).catch(() => {});
                            }, 500);
                        }
                    }
                    navigationTrigger = 0;
                }
            }
        });
    }

    // Call init but don't await it here to prevent blocking the return
    init();

    // --- Actions ---
    return {
        // State getters
        get hosts() { return hosts; },
        get sessions() { return sessions; },
        get activeSessionId() { return activeSessionId; },
        set activeSessionId(id: string | null) { activeSessionId = id; },
        get activeSession() { return activeSession; },
        get activeProject() { return activeProject; },
        get recentSessions() { return recentSessions; },
        get terminalFontSize() { return terminalFontSize; },
        get terminalFontWeight() { return terminalFontWeight; },
        get markdownFontSize() { return markdownFontSize; },
        get markdownFontWeight() { return markdownFontWeight; },
        get terminalFocusTrigger() { return terminalFocusTrigger; },
        get terminalResizeTrigger() { return terminalResizeTrigger; },
        get navigationTrigger() { return navigationTrigger; },
        get openMarkdownFiles() { return openMarkdownFiles; },
        get projectMdFiles() { return projectMdFiles; },
        get loadedFiles() { return loadedFiles; },
        get logs() { return logs; },
        get sshKeys() { return sshKeys; },
        get viewUpdateTrigger() { return viewUpdateTrigger; },
        get daemonConnected() { return daemonConnected; },
        get daemonRecentSessions() { return daemonRecentSessions; },
        get lastAgentNotification() { return lastAgentNotification; },
        get pinnedProjects() { return pinnedProjects; },

        // Spread the methods
        ...methods
    };
}

export const appState = createAppState();
