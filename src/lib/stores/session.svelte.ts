import { load } from '@tauri-apps/plugin-store';

export type TabType = 'terminal' | 'viewer';

export interface Tab {
    id: string;
    title: string;
    type: TabType;
    data: any;
}

class SessionStore {
    tabs = $state<Tab[]>([]);
    activeTabIndex = $state(0);
    private store: any;

    constructor() {
        this.init();
    }

    async init() {
        try {
            this.store = await load('settings.json', { autoSave: true });
            const savedTabs = await this.store.get('tabs');
            if (savedTabs && Array.isArray(savedTabs)) {
                this.tabs = savedTabs;
            } else {
                this.addTab({
                    id: 'initial',
                    title: 'Terminal 1',
                    type: 'terminal',
                    data: { host: '', username: '', password: '', connected: false }
                });
            }
        } catch (e) {
            console.error('Failed to load store:', e);
            // Fallback
            this.tabs = [{
                id: 'initial',
                title: 'Terminal 1',
                type: 'terminal',
                data: { host: '', username: '', password: '', connected: false }
            }];
        }
    }

    async save() {
        if (this.store) {
            await this.store.set('tabs', Array.from(this.tabs));
        }
    }

    addTab(tab: Tab) {
        if (tab.type === 'terminal' && !tab.data) {
            tab.data = { host: '', username: '', password: '', connected: false };
        }
        this.tabs.push(tab);
        this.activeTabIndex = this.tabs.length - 1;
        this.save();
    }

    closeTab(id: string) {
        if (this.tabs.length <= 1) return;
        
        const index = this.tabs.findIndex(t => t.id === id);
        if (index !== -1) {
            this.tabs.splice(index, 1);
            if (this.activeTabIndex >= this.tabs.length) {
                this.activeTabIndex = this.tabs.length - 1;
            }
            this.save();
        }
    }

    setActiveTab(index: number) {
        if (index >= 0 && index < this.tabs.length) {
            this.activeTabIndex = index;
        }
    }
}

export const sessionStore = new SessionStore();