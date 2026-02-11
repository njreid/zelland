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
    error?: string; // Connection error description
    projects: Project[];
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

export interface AppState {
    hosts: Host[];
    sessions: Session[];
    activeSessionId: string | null;
    status: 'disconnected' | 'connecting' | 'connected' | 'error';
    error: string | null;
    terminalFocusTrigger: number;
    terminalResizeTrigger: number;
    viewUpdateTrigger: Record<string, number>;
    loadedFiles: Record<string, boolean>;
    terminalFontSize: number;
    logs: { message: string, type: 'info' | 'error' }[];
    sshKeys: any[];
}

const STORE_PATH = "settings.json";

function createAppState() {
    let state = $state<AppState>({
        hosts: [],
        sessions: [],
        activeSessionId: null,
        status: 'disconnected',
        error: null,
        terminalFocusTrigger: 0,
        terminalResizeTrigger: 0,
        viewUpdateTrigger: {},
        loadedFiles: {},
        terminalFontSize: 14,
        logs: [],
        sshKeys: [],
    });

    function log(message: string, type: 'info' | 'error' = 'info') {
        state.logs.push({ message, type });
        if (state.logs.length > 50) state.logs.shift();
    }

    async function fetchSshKeys() {
        try {
            state.sshKeys = await invoke<any[]>("list_ssh_keys");
        } catch (e) {
            console.error("Failed to fetch SSH keys:", e);
        }
    }

    // Listen for events from daemon
    listen("daemon-event", (event: any) => {
        const payload = event.payload.payload;
        if (payload && payload.OpenView) {
            const req = payload.OpenView;
            log(`Received view update: ${req.title}`);
            if (!state.viewUpdateTrigger[req.title]) {
                state.viewUpdateTrigger[req.title] = 0;
            }
            state.viewUpdateTrigger[req.title] += 1;
        }
    });

    async function fetchProjectsForHost(hostId: string) {
        const host = state.hosts.find(h => h.id === hostId);
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

    async function saveToStore() {
        try {
            const store = await load(STORE_PATH);
            const hostsToSave = state.hosts.map(({ id, label, address, username, password }) => ({
                id, label, address, username, password
            }));
            const sessionsToSave = state.sessions.map(({ id, label, hostAddress, username, password, port, type, zellijSession, key_id, private_key_path }) => ({
                id, label, hostAddress, username, password, port, type, zellijSession, key_id, private_key_path
            }));

            await store.set("hosts", hostsToSave);
            await store.set("sessions", sessionsToSave);
            await store.set("terminalFontSize", state.terminalFontSize);
            await store.save();
        } catch (e) {
            console.error("Failed to save to store:", e);
        }
    }

    // Initialize store and load data
    async function initStore() {
        try {
            const store = await load(STORE_PATH);
            const savedHosts = await store.get<Host[]>("hosts");
            const savedSessions = await store.get<Session[]>("sessions");
            const savedFontSize = await store.get<number>("terminalFontSize");

            if (savedHosts) {
                state.hosts = savedHosts.map(h => ({ ...h, reachable: false, projects: [] }));
            }
            if (savedSessions) {
                state.sessions = savedSessions.map(s => ({ ...s, status: 'disconnected' }));
            }
            if (savedFontSize) {
                state.terminalFontSize = savedFontSize;
            }
            
            for (const host of state.hosts) {
                fetchProjectsForHost(host.id);
            }

            fetchSshKeys();
        } catch (e) {
            console.error("Failed to load store:", e);
        }
    }

    initStore();

    // Listen for tunnel status events from Rust
    listen("tunnel-status", (event) => {
        log(`Tunnel status: ${event.payload}`);
    });

    listen("tunnel-error", (event) => {
        log(`Tunnel error: ${event.payload}`, 'error');
    });

    // Listen for biometric authentication requests from the Rust backend (Android)
    listen("biometric-request", async (event: any) => {
        const request = event.payload;
        log(`Biometric auth requested for key: ${request.key_id}`);
        try {
            // On Android, this would trigger the native BiometricPrompt via JNI.
            // The result is sent back to Rust via the biometric_result command.
            // For now on desktop, we auto-approve (no biometric hardware).
            await invoke("biometric_result", {
                response: {
                    request_id: request.request_id,
                    success: true,
                    error: null
                }
            });
        } catch (e) {
            console.error("Failed to handle biometric request:", e);
            await invoke("biometric_result", {
                response: {
                    request_id: request.request_id,
                    success: false,
                    error: String(e)
                }
            });
        }
    });

    return {
        get hosts() { return state.hosts; },
        get sessions() { return state.sessions; },
        get activeSessionId() { return state.activeSessionId; },
        set activeSessionId(id: string | null) { state.activeSessionId = id; },
        get terminalFocusTrigger() { return state.terminalFocusTrigger; },
        get terminalResizeTrigger() { return state.terminalResizeTrigger; },
        get terminalFontSize() { return state.terminalFontSize; },
        get loadedFiles() { return state.loadedFiles; },
        get logs() { return state.logs; },
        get viewUpdateTrigger() { return state.viewUpdateTrigger; },
        get sshKeys() { return state.sshKeys; },
        get currentProject() { return null; },

        closeActiveSession() {
            state.activeSessionId = null;
        },

        setTerminalFontSize(size: number) {
            state.terminalFontSize = size;
            saveToStore();
            // Trigger a resize to adapt to new font size
            this.triggerTerminalResize();
        },

        setFileLoaded(filename: string, loaded: boolean) {
            state.loadedFiles[filename] = loaded;
        },

        triggerTerminalFocus() {
            state.terminalFocusTrigger += 1;
        },

        triggerTerminalResize() {
            state.terminalResizeTrigger += 1;
        },

        async generateSshKey(label: string) {
            try {
                await invoke("generate_ssh_key", { label });
                await fetchSshKeys();
            } catch (e) {
                console.error("Failed to generate SSH key:", e);
                log(`Failed to generate SSH key: ${e}`, 'error');
            }
        },

        async deleteSshKey(id: string) {
            try {
                await invoke("delete_ssh_key", { id });
                await fetchSshKeys();
            } catch (e) {
                console.error("Failed to delete SSH key:", e);
                log(`Failed to delete SSH key: ${e}`, 'error');
            }
        },

        async fetchSshKeys() {
            await fetchSshKeys();
        },

        async addHost(label: string, address: string, username: string, password?: string) {
            const id = crypto.randomUUID();
            state.hosts.push({ 
                id, label, address, username, password,
                reachable: false, projects: [] 
            });
            await saveToStore();
            await fetchProjectsForHost(id);
        },

        async fetchProjectsForHost(hostId: string) {
            await fetchProjectsForHost(hostId);
        },

        async fetchAllProjects() {
            for (const host of state.hosts) {
                await fetchProjectsForHost(host.id);
            }
        },

        async activateProject(hostId: string, projectId: string) {
            const host = state.hosts.find(h => h.id === hostId);
            if (!host) return;

            const project = host.projects.find(p => p.id === projectId);
            if (!project) return;

            const sessionId = `project-${project.id}`;
            let session = state.sessions.find(s => s.id === sessionId);
            
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
                state.sessions.push(session);
                await saveToStore();
            }

            await this.connectSession(sessionId);
        },

        async addSession(label: string, hostAddress: string, username: string, type: 'ssh', zellijSession: string, password?: string, keyId?: string, privateKeyPath?: string) {
            const id = crypto.randomUUID();
            state.sessions.push({
                id, label, hostAddress, username, password,
                port: 22, type, zellijSession, status: 'disconnected',
                key_id: keyId,
                private_key_path: privateKeyPath
            });
            await saveToStore();
        },

        async removeHost(hostId: string) {
            state.hosts = state.hosts.filter(h => h.id !== hostId);
            await saveToStore();
        },

        async removeSession(sessionId: string) {
            state.sessions = state.sessions.filter(s => s.id !== sessionId);
            if (state.activeSessionId === sessionId) {
                state.activeSessionId = null;
            }
            await saveToStore();
        },

        async connectSession(sessionId: string) {
            const session = state.sessions.find(s => s.id === sessionId);
            if (!session) return;

            session.status = 'connecting';
            state.activeSessionId = sessionId;
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

                try {
                    const wsUrl = `ws://${session.hostAddress}:8083/ws`;
                    await invoke("daemon_connect", { url: wsUrl });
                    log(`Connected to daemon at ${session.hostAddress}`);
                } catch (de) {
                    console.warn("Daemon WebSocket connection failed:", de);
                }
            } catch (e) {
                const err = String(e);
                console.error("Connection failed:", e);
                session.status = 'error';
                state.error = err;
                log(`Connection failed for ${session.label}: ${err}`, 'error');
            }
        },

        async writeInput(sessionId: string, data: number[]) {
            const session = state.sessions.find(s => s.id === sessionId);
            if (!session) return;

            try {
                await invoke("ssh_write", { tabId: sessionId, data });
            } catch (e) {
                console.error("Failed to write input:", e);
            }
        },

        async runZellijAction(sessionId: string, action: string) {
            const session = state.sessions.find(s => s.id === sessionId);
            if (!session) return;

            const authMethod = session.key_id ? "Key" : session.private_key_path ? "PrivateKey" : "Password";
            const config = {
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

            const command = `zellij -s ${session.zellijSession} action ${action}`;
            try {
                await invoke("run_remote_command", { config, command });
            } catch (e) {
                console.error("Failed to run zellij action:", e);
                log(`Failed to run zellij action: ${e}`, 'error');
            }
        },

        async resize(sessionId: string, rows: number, cols: number) {
            if (!sessionId) return;
            const session = state.sessions.find(s => s.id === sessionId);
            if (!session) return;

            try {
                await invoke("ssh_resize", { tabId: sessionId, rows, cols });
            } catch (e) {
                console.error("Failed to resize session:", e);
            }
        }
    };
}

export const appState = createAppState();