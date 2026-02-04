<script lang="ts">
    import TerminalComponent from '$lib/components/Terminal.svelte';
    import VirtualKeyboard from '$lib/components/VirtualKeyboard.svelte';
    import Viewer from '$lib/components/Viewer.svelte';
    import { sessionStore } from '$lib/stores/session.svelte';
    import { onMount } from 'svelte';
    import { listen } from '@tauri-apps/api/event';
    import { invoke } from '@tauri-apps/api/core';

    onMount(async () => {
        // Handle incoming intents from Android
        await listen('intent-received', (event: any) => {
            console.log('Received intent:', event.payload.text);
            // Auto-paste into active terminal if it's connected
            const activeTab = sessionStore.tabs[sessionStore.activeTabIndex];
            if (activeTab && activeTab.type === 'terminal' && activeTab.data.connected) {
                invoke('ssh_write', { tabId: activeTab.id, data: Array.from(new TextEncoder().encode(event.payload.text)) });
            }
        });

        // Handle daemon events
        await listen('daemon-event', (event: any) => {
            console.log('Received daemon event:', event.payload);
            const payload = event.payload.payload;
            
            if (payload && payload.open_view) {
                const view = payload.open_view;
                sessionStore.addTab({
                    id: view.asset_id || Math.random().toString(),
                    title: view.title || 'Viewer',
                    type: 'viewer',
                    data: {
                        url: view.url,
                        fileType: view.file_type
                    }
                });
            }
        });
    });
</script>

<div class="flex flex-col h-screen bg-[#1a1b26] text-[#a9b1d6] overflow-hidden">
    <!-- Tab Bar -->
    <div class="flex bg-[#16161e] border-b border-[#313244] overflow-x-auto scrollbar-hide">
        {#each sessionStore.tabs as tab, i}
            <div class="relative group">
                <button 
                    class="px-4 py-2 text-sm whitespace-nowrap transition-colors {sessionStore.activeTabIndex === i ? 'bg-[#1a1b26] border-b-2 border-[#7aa2f7] text-[#7aa2f7]' : 'text-[#565f89] hover:text-[#a9b1d6]'}"
                    onclick={() => sessionStore.setActiveTab(i)}
                >
                    {tab.title}
                </button>
                {#if sessionStore.tabs.length > 1}
                    <button 
                        class="absolute right-1 top-1 opacity-0 group-hover:opacity-100 text-xs text-[#f7768e]"
                        onclick={(e) => { e.stopPropagation(); sessionStore.closeTab(tab.id); }}
                    >
                        ×
                    </button>
                {/if}
            </div>
        {/each}
        <button 
            class="px-4 py-2 text-[#565f89] hover:text-[#a9b1d6]"
            onclick={() => sessionStore.addTab({ id: Math.random().toString(), title: `Terminal ${sessionStore.tabs.length + 1}`, type: 'terminal', data: {} })}
        >
            +
        </button>
    </div>

    <!-- Active Tab Content -->
    <div class="flex-1 relative overflow-hidden">
        {#each sessionStore.tabs as tab, i}
            <div class="absolute inset-0 {sessionStore.activeTabIndex === i ? 'block' : 'hidden'}">
                {#if tab.type === 'terminal'}
                    {#if !tab.data.connected}
                        <div class="flex flex-col items-center justify-center h-full p-6 space-y-4 bg-[#1a1b26]">
                            <h2 class="text-xl font-bold text-[#7aa2f7]">Connect to Remote Host</h2>
                            <div class="w-full max-w-sm space-y-2">
                                <input type="text" placeholder="Host (e.g. 192.168.1.5)" class="w-full px-4 py-2 rounded bg-[#24283b] border border-[#414868] text-white outline-none focus:border-[#7aa2f7]" bind:value={tab.data.host} />
                                <input type="text" placeholder="Username" class="w-full px-4 py-2 rounded bg-[#24283b] border border-[#414868] text-white outline-none focus:border-[#7aa2f7]" bind:value={tab.data.username} />
                                <input type="password" placeholder="Password" class="w-full px-4 py-2 rounded bg-[#24283b] border border-[#414868] text-white outline-none focus:border-[#7aa2f7]" bind:value={tab.data.password} />
                                <button 
                                    class="w-full py-2 mt-4 rounded bg-[#7aa2f7] text-[#1a1b26] font-bold active:opacity-80 transition-opacity"
                                    onclick={async () => {
                                        try {
                                            await invoke('ssh_connect', { 
                                                tabId: tab.id,
                                                config: {
                                                    host: tab.data.host,
                                                    port: 22,
                                                    username: tab.data.username,
                                                    auth_method: 'Password',
                                                    password: tab.data.password,
                                                    private_key_path: null,
                                                    private_key_passphrase: null
                                                } 
                                            });
                                            tab.data.connected = true;
                                        } catch (e) {
                                            alert('Connection failed: ' + e);
                                        }
                                    }}
                                >
                                    Connect
                                </button>
                            </div>
                        </div>
                    {:else}
                        <TerminalComponent tabId={tab.id} data={tab.data} />
                    {/if}
                {:else if tab.type === 'viewer'}
                    <Viewer title={tab.title} url={tab.data.url} fileType={tab.data.fileType} />
                {/if}
            </div>
        {/each}
    </div>

    <!-- Virtual Keyboard -->
    <VirtualKeyboard />
</div>

<style>
    :global(body) {
        margin: 0;
        padding: 0;
        overflow: hidden;
    }
    
    .scrollbar-hide::-webkit-scrollbar {
        display: none;
    }
    .scrollbar-hide {
        -ms-overflow-style: none;
        scrollbar-width: none;
    }
</style>
