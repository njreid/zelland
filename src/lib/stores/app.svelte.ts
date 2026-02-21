import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { load } from "@tauri-apps/plugin-store";

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
    password?: string;
    reachable: boolean;
    error?: string;
    projects: Project[];
}

export interface ZellijTab {
    index: number;
    name: string;
    active: boolean;
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
    let openMarkdownFiles = $state<string[]>(['README.md', 'PLAN.md', 'DESIGN.md']);
    let navigationTrigger = $state(-1);
    let recentSessionIds = $state<string[]>([]);
    let terminalFontSize = $state(14);
    let terminalFontWeight = $state('400');
    let markdownFontSize = $state(16);
    let markdownFontWeight = $state('400');
    let logs = $state<{ message: string, type: 'info' | 'error' }[]>([]);
    let sshKeys = $state<any[]>([]);
    let daemonConnected = $state(false);
    let zellijTabs = $state<ZellijTab[]>([]);
    let tabPollInterval: ReturnType<typeof setInterval> | null = null;

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

    // --- Helpers ---
    function log(message: string, type: 'info' | 'error' = 'info') {
        logs.push({ message, type });
        if (logs.length > 50) logs.shift();
    }

    function buildSshConfig(session: Session) {
        const authMethod = session.key_id ? "Key" : session.private_key_path ? "PrivateKey" : "Password";
        return {
            host: session.hostAddress,
            port: session.port,
            username: session.username,
            auth_method: authMethod,
            password: session.password || null,
            private_key_path: session.private_key_path || null,
            private_key_passphrase: null,
            key_id: session.key_id || null,
            session_name: session.zellijSession
        };
    }

    async function fetchZellijTabs() {
        const session = activeSession;
        if (!session || session.status !== 'connected') return;

        const command = [
            `zellij -s ${session.zellijSession}`,
            `pipe --plugin file:~/.config/zellij/plugins/zelland-tabs.wasm`,
            `--name list-tabs 2>/dev/null`
        ].join(' ');

        try {
            const result = await invoke<string>("run_remote_command", {
                config: buildSshConfig(session),
                command
            });
            const trimmed = result.trim();
            if (trimmed.startsWith('[')) {
                zellijTabs = JSON.parse(trimmed);
            }
        } catch {
            // Plugin not yet installed or session not ready — silent fail
        }
    }

    function startTabPolling() {
        stopTabPolling();
        fetchZellijTabs();
        tabPollInterval = setInterval(fetchZellijTabs, 2000);
    }

    function stopTabPolling() {
        if (tabPollInterval !== null) {
            clearInterval(tabPollInterval);
            tabPollInterval = null;
        }
        zellijTabs = [];
    }

    async function saveToStore() {
        try {
            const store = await load(STORE_PATH);
            await store.set("hosts", hosts.map(({ id, label, address, username, password }) => ({
                id, label, address, username, password
            })));
            await store.set("sessions", sessions.map(({ id, label, hostAddress, username, password, port, type, zellijSession, key_id, private_key_path }) => ({
                id, label, hostAddress, username, password, port, type, zellijSession, key_id, private_key_path
            })));
            await store.set("terminalFontSize", terminalFontSize);
            await store.set("terminalFontWeight", terminalFontWeight);
            await store.set("markdownFontSize", markdownFontSize);
            await store.set("markdownFontWeight", markdownFontWeight);
            await store.set("recentSessionIds", recentSessionIds);
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

        try {
            log(`Fetching projects for ${host.label}...`);
            const projects = await invoke<any[]>("daemon_get_projects", { url: `http://${host.address}:8083` });
            host.projects = projects.map(p => ({ ...p, hostId: host.id }));
            host.reachable = true;
            host.error = undefined;
            log(`Fetched ${projects.length} projects for ${host.label}.`);
        } catch (e) {
            const err = String(e);
            console.error(`Failed to fetch projects for host ${host.label}:`, e);
            host.reachable = false;
            host.error = err;
            log(`Failed to fetch projects for ${host.label}: ${err}`, 'error');
        }
    }

    // --- Initialization ---
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

            if (savedHosts) hosts = savedHosts.map(h => ({ ...h, reachable: false, projects: [] }));
            if (savedSessions) sessions = savedSessions.map(s => ({ ...s, status: 'disconnected' }));
            if (savedFontSize) terminalFontSize = savedFontSize;
            if (savedFontWeight) terminalFontWeight = savedFontWeight;
            if (savedMarkdownFontSize) markdownFontSize = savedMarkdownFontSize;
            if (savedMarkdownFontWeight) markdownFontWeight = savedMarkdownFontWeight;
            if (savedRecentIds) recentSessionIds = savedRecentIds;
            
            for (const host of hosts) fetchProjectsForHost(host.id);
            fetchSshKeys();
        } catch (e) {
            console.error("Failed to load store:", e);
        }

        // --- Event Listeners ---
        listen("daemon-event", (event: any) => {
            const payload = event.payload.payload;
            if (payload?.OpenView) {
                const req = payload.OpenView;
                log(`Received view update: ${req.title}`);
                if (!viewUpdateTrigger[req.title]) viewUpdateTrigger[req.title] = 0;
                viewUpdateTrigger[req.title] += 1;
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
    }

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
        get loadedFiles() { return loadedFiles; },
        get logs() { return logs; },
        get sshKeys() { return sshKeys; },
        get viewUpdateTrigger() { return viewUpdateTrigger; },
        get daemonConnected() { return daemonConnected; },
        get zellijTabs() { return zellijTabs; },

        // Methods
        scrollToPane(index: number) { navigationTrigger = index; },
        
        openMarkdownFile(filename: string) {
            const cleanName = filename.startsWith('./') ? filename.slice(2) : filename;
            let index = openMarkdownFiles.indexOf(cleanName);
            if (index === -1) {
                openMarkdownFiles.push(cleanName);
                index = openMarkdownFiles.length - 1;
            }
            this.scrollToPane(index + 1);
        },

        triggerTerminalFocus() { terminalFocusTrigger += 1; },
        triggerTerminalResize() { terminalResizeTrigger += 1; },
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

        async addHost(label: string, address: string, username: string, password?: string) {
            const id = crypto.randomUUID();
            hosts.push({ id, label, address, username, password, reachable: false, projects: [] });
            await saveToStore();
            fetchProjectsForHost(id);
        },

        async fetchProjectsForHost(hostId: string) { await fetchProjectsForHost(hostId); },

        async fetchAllProjects() {
            for (const host of hosts) await fetchProjectsForHost(host.id);
        },

        async removeHost(hostId: string) {
            hosts = hosts.filter(h => h.id !== hostId);
            await saveToStore();
        },

        async addSession(label: string, hostAddress: string, username: string, type: 'ssh', zellijSession: string, password?: string, keyId?: string, privateKeyPath?: string) {
            const id = crypto.randomUUID();
            sessions.push({
                id, label, hostAddress, username, password,
                port: 22, type, zellijSession, status: 'disconnected',
                key_id: keyId, private_key_path: privateKeyPath
            });
            await saveToStore();
        },

        async removeSession(sessionId: string) {
            if (activeSessionId === sessionId) stopTabPolling();
            sessions = sessions.filter(s => s.id !== sessionId);
            if (activeSessionId === sessionId) activeSessionId = null;
            await saveToStore();
        },

        async connectSession(sessionId: string) {
            const session = sessions.find(s => s.id === sessionId);
            if (!session) return;

            stopTabPolling(); // clear any previous session's polling
            session.status = 'connecting';
            activeSessionId = sessionId;
            log(`Connecting to session: ${session.label}...`);

            try {
                const authMethod = session.key_id ? "Key" : session.private_key_path ? "PrivateKey" : "Password";
                await invoke('ssh_connect', {
                    tabId: session.id,
                    config: {
                        host: session.hostAddress,
                        port: session.port,
                        username: session.username,
                        auth_method: authMethod,
                        password: session.password || null,
                        private_key_path: session.private_key_path || null,
                        private_key_passphrase: null,
                        key_id: session.key_id || null,
                        session_name: session.zellijSession
                    }
                });
                
                session.status = 'connected';
                log(`Connected to session: ${session.label}.`);
                startTabPolling();

                recentSessionIds = [sessionId, ...recentSessionIds.filter(id => id !== sessionId)].slice(0, 3);
                saveToStore();

                try {
                    await invoke("daemon_connect", { url: `ws://${session.hostAddress}:8083/ws` });
                    daemonConnected = true;
                    log(`Connected to daemon at ${session.hostAddress}`);
                } catch (de) {
                    daemonConnected = false;
                    console.warn("Daemon WebSocket connection failed:", de);
                }
            } catch (e) {
                daemonConnected = false;
                session.status = 'error';
                log(`Connection failed for ${session.label}: ${e}`, 'error');
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
                    port: 22,
                    type: 'ssh',
                    zellijSession: project.session_name,
                    status: 'disconnected'
                };
                sessions.push(session);
                await saveToStore();
            }

            await this.connectSession(sessionId);
        },

        async writeInput(sessionId: string, data: number[]) {
            try {
                await invoke("ssh_write", { tabId: sessionId, data });
            } catch (e) {
                console.error("Failed to write input:", e);
            }
        },

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
                await invoke("run_remote_command", { config: buildSshConfig(session), command });
            } catch (e) {
                log(`Failed to run zellij action: ${e}`, 'error');
            }
        },

        async resize(sessionId: string, rows: number, cols: number) {
            const session = sessions.find(s => s.id === sessionId);
            if (session?.status === 'connected') {
                try {
                    await invoke("ssh_resize", { tabId: sessionId, rows, cols });
                } catch (e) {
                    console.error("Failed to resize session:", e);
                }
            }
        }
    };
}

export const appState = createAppState();
