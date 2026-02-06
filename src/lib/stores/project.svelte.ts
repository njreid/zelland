import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export interface Project {
    id: string;
    name?: string;
    host: string;
    session_name: string;
    root_path: string;
}

export interface ProjectState {
    projects: Project[];
    currentProject: Project | null;
    status: 'disconnected' | 'connecting' | 'connected' | 'error';
    error: string | null;
    tunnelStatus: 'disconnected' | 'connected' | 'error';
}

function createProjectState() {
    let state = $state<ProjectState>({
        projects: [],
        currentProject: null,
        status: 'disconnected',
        error: null,
        tunnelStatus: 'disconnected'
    });

    // Listen for tunnel status events from Rust
    listen("tunnel-status", (event) => {
        state.tunnelStatus = event.payload as any;
    });

    listen("tunnel-error", (event) => {
        state.tunnelStatus = 'error';
        state.error = event.payload as string;
    });

    return {
        get projects() { return state.projects; },
        get currentProject() { return state.currentProject; },
        get status() { return state.status; },
        get error() { return state.error; },
        get tunnelStatus() { return state.tunnelStatus; },

        async fetchProjects(daemonUrl: string) {
            try {
                state.projects = await invoke("daemon_get_projects", { url: daemonUrl });
            } catch (e) {
                state.error = String(e);
            }
        },

        async activateProject(daemonUrl: string, projectId: string) {
            try {
                await invoke("daemon_activate_project", { url: daemonUrl, projectId });
                state.currentProject = state.projects.find(p => p.id === projectId) || null;
                state.status = 'connected';
            } catch (e) {
                state.error = String(e);
                state.status = 'error';
            }
        },

        setProject(project: Project | null) {
            state.currentProject = project;
        }
    };
}

export const projectStore = createProjectState();
