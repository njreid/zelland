<script lang="ts">
    import TerminalComponent from '$lib/components/Terminal.svelte';
    import VirtualKeyboard from '$lib/components/VirtualKeyboard.svelte';
    import { sessionStore } from '$lib/stores/session.svelte';
    import { onMount } from 'svelte';
    import { listen } from '@tauri-apps/api/event';

    onMount(async () => {
        // Handle incoming intents from Android
        await listen('intent-received', (event: any) => {
            console.log('Received intent:', event.payload.text);
            // TODO: Prompt user to paste or open
        });

        // Handle daemon events
        await listen('daemon-event', (event: any) => {
            console.log('Received daemon event:', event.payload);
            // TODO: Handle OpenViewRequest etc.
        });
    });
</script>

<div class="flex flex-col h-screen bg-[#1a1b26] text-[#a9b1d6] overflow-hidden">
    <!-- Tab Bar -->
    <div class="flex bg-[#16161e] border-b border-[#313244] overflow-x-auto scrollbar-hide">
        {#each sessionStore.tabs as tab, i}
            <button 
                class="px-4 py-2 text-sm whitespace-nowrap transition-colors {sessionStore.activeTabIndex === i ? 'bg-[#1a1b26] border-b-2 border-[#7aa2f7] text-[#7aa2f7]' : 'text-[#565f89] hover:text-[#a9b1d6]'}"
                onclick={() => sessionStore.setActiveTab(i)}
            >
                {tab.title}
            </button>
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
                    <TerminalComponent tabId={tab.id} data={tab.data} />
                {:else}
                    <div class="flex items-center justify-center h-full">
                        Viewer for {tab.title}
                    </div>
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