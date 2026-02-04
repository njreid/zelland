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

    constructor() {
        // Initial tab
        this.addTab({
            id: 'initial',
            title: 'Terminal 1',
            type: 'terminal',
            data: {}
        });
    }

    addTab(tab: Tab) {
        this.tabs.push(tab);
        this.activeTabIndex = this.tabs.length - 1;
    }

    closeTab(id: string) {
        if (this.tabs.length <= 1) return;
        
        const index = this.tabs.findIndex(t => t.id === id);
        if (index !== -1) {
            this.tabs.splice(index, 1);
            if (this.activeTabIndex >= this.tabs.length) {
                this.activeTabIndex = this.tabs.length - 1;
            }
        }
    }

    setActiveTab(index: number) {
        if (index >= 0 && index < this.tabs.length) {
            this.activeTabIndex = index;
        }
    }
}

export const sessionStore = new SessionStore();
