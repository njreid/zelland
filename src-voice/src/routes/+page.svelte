<script lang="ts">
    import VoiceTerminal from '$lib/components/VoiceTerminal.svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event';
    import { onMount, onDestroy } from 'svelte';

    let status = $state('Initializing...');
    let recording = $state(false);
    let previewText = $state('');
    let showPreview = $state(false);
    let previewInput: HTMLInputElement = $state(null!);
    let terminalRef: { sendToPty: (text: string) => void } | undefined = $state();
    let speechReady = $state(false);
    let ptyClosed = $state(false);
    let unlisteners: (() => void)[] = [];

    // Audio settings
    let showSettings = $state(false);
    let audioDevices: { name: string; is_default: boolean }[] = $state([]);
    let selectedDevice: string | null = $state(null);
    let inputGain = $state(1.0);

    onMount(async () => {
        const u1 = await listen('speech-ready', () => {
            speechReady = true;
            if (!recording) status = 'Alt+Space: voice input';
        });
        unlisteners.push(u1);

        const u2 = await listen<string>('speech-error', (event) => {
            console.error('Speech error:', event.payload);
            status = `Error: ${event.payload}`;
            setTimeout(() => {
                status = speechReady ? 'Alt+Space: voice input' : 'Speech engine not loaded';
            }, 3000);
        });
        unlisteners.push(u2);

        const u3 = await listen('pty-closed', () => {
            ptyClosed = true;
            status = 'Shell exited. Ctrl+Shift+R to restart.';
        });
        unlisteners.push(u3);

        // Init speech engine (non-blocking — terminal works without it)
        try {
            await invoke('init_speech');
        } catch (e: any) {
            console.error('Speech init error:', e);
            const msg = String(e);
            if (msg.includes('not found') || msg.includes('No such file')) {
                status = 'No model — run: bash scripts/download-model.sh';
            } else {
                status = 'Alt+Space: voice input (no model)';
            }
        }

        if (!speechReady && !status.includes('No model')) {
            status = 'Alt+Space: voice input (loading...)';
        }
    });

    onDestroy(() => {
        unlisteners.forEach(u => u());
    });

    async function toggleRecording() {
        if (!speechReady) return;

        if (!recording) {
            try {
                await invoke('start_recording');
                recording = true;
                status = 'Recording... (Alt+Space to stop)';
            } catch (e: any) {
                console.error('Recording error:', e);
                const msg = String(e);
                if (msg.includes('No input audio device')) {
                    status = 'No microphone detected';
                } else {
                    status = `Mic error: ${msg}`;
                }
                setTimeout(() => { status = 'Alt+Space: voice input'; }, 3000);
            }
        } else {
            recording = false;
            status = 'Transcribing...';
            try {
                const text: string = await invoke('stop_and_transcribe');
                if (text.trim()) {
                    previewText = text.trim();
                    showPreview = true;
                    status = 'Enter: send | Esc: cancel';
                    requestAnimationFrame(() => previewInput?.focus());
                } else {
                    status = 'Alt+Space: voice input';
                }
            } catch (e: any) {
                console.error('Transcription error:', e);
                status = `Transcription error: ${e}`;
                setTimeout(() => { status = 'Alt+Space: voice input'; }, 3000);
            }
        }
    }

    async function restartPty() {
        try {
            await invoke('pty_spawn');
            ptyClosed = false;
            status = speechReady ? 'Alt+Space: voice input' : status;
        } catch (e: any) {
            status = `Failed to restart shell: ${e}`;
        }
    }

    async function toggleSettings() {
        showSettings = !showSettings;
        if (showSettings) {
            try {
                audioDevices = await invoke('list_audio_devices');
            } catch (e: any) {
                console.error('Failed to list devices:', e);
                audioDevices = [];
            }
        }
    }

    async function onDeviceChange(e: Event) {
        const value = (e.target as HTMLSelectElement).value;
        selectedDevice = value || null;
        try {
            await invoke('set_audio_device', { deviceName: selectedDevice });
        } catch (err: any) {
            console.error('Failed to set device:', err);
        }
    }

    async function onGainChange(e: Event) {
        inputGain = parseFloat((e.target as HTMLInputElement).value);
        try {
            await invoke('set_input_gain', { gain: inputGain });
        } catch (err: any) {
            console.error('Failed to set gain:', err);
        }
    }

    function handlePreviewKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') {
            e.preventDefault();
            if (previewText.trim() && terminalRef) {
                terminalRef.sendToPty(previewText + '\n');
            }
            showPreview = false;
            previewText = '';
            status = 'Alt+Space: voice input';
        } else if (e.key === 'Escape') {
            e.preventDefault();
            showPreview = false;
            previewText = '';
            status = 'Alt+Space: voice input';
        }
    }

    function handleGlobalKeydown(e: KeyboardEvent) {
        if (e.key === ' ' && e.altKey) {
            e.preventDefault();
            toggleRecording();
        }
        // Ctrl+Shift+R to restart PTY
        if (e.key === 'R' && e.ctrlKey && e.shiftKey && ptyClosed) {
            e.preventDefault();
            restartPty();
        }
    }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="app">
    <div class="terminal-area">
        <VoiceTerminal bind:this={terminalRef} />
    </div>

    {#if showPreview}
        <div class="preview-bar">
            <span class="preview-label">Preview:</span>
            <input
                bind:this={previewInput}
                bind:value={previewText}
                onkeydown={handlePreviewKeydown}
                class="preview-input"
                type="text"
                spellcheck="false"
            />
        </div>
    {/if}

    {#if showSettings}
        <div class="settings-panel">
            <label class="settings-label">
                Mic
                <select class="settings-select" onchange={onDeviceChange} value={selectedDevice ?? ''}>
                    <option value="">Default</option>
                    {#each audioDevices as dev}
                        <option value={dev.name}>
                            {dev.name}{dev.is_default ? ' (default)' : ''}
                        </option>
                    {/each}
                </select>
            </label>
            <label class="settings-label">
                Gain {inputGain.toFixed(1)}
                <input
                    type="range"
                    min="0"
                    max="2"
                    step="0.1"
                    value={inputGain}
                    oninput={onGainChange}
                    class="gain-slider"
                />
            </label>
        </div>
    {/if}

    <div class="status-bar">
        {#if recording}
            <span class="recording-dot"></span>
        {/if}
        <span class="status-text">{status}</span>
        {#if ptyClosed}
            <button class="restart-btn" onclick={restartPty}>Restart Shell</button>
        {/if}
        <button class="settings-btn" onclick={toggleSettings} title="Audio settings">
            {showSettings ? '\u2715' : '\u2699'}
        </button>
    </div>
</div>

<style>
    .app {
        display: flex;
        flex-direction: column;
        height: 100vh;
        width: 100vw;
    }

    .terminal-area {
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    .preview-bar {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 6px 12px;
        background: var(--bg-input);
        border-top: 1px solid var(--border);
    }

    .preview-label {
        color: var(--accent);
        font-size: 12px;
        white-space: nowrap;
    }

    .preview-input {
        flex: 1;
        background: transparent;
        border: none;
        outline: none;
        color: var(--fg);
        font-family: 'Inconsolata', 'Consolas', monospace;
        font-size: 14px;
    }

    .status-bar {
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 4px 12px;
        background: var(--bg-dark);
        border-top: 1px solid var(--border);
        min-height: 28px;
    }

    .recording-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: var(--error);
        animation: pulse 1s ease-in-out infinite;
    }

    @keyframes pulse {
        0%, 100% { opacity: 1; }
        50% { opacity: 0.3; }
    }

    .status-text {
        color: var(--fg-dim);
        font-size: 12px;
    }

    .restart-btn {
        margin-left: auto;
        padding: 2px 10px;
        background: var(--accent);
        color: var(--bg);
        border: none;
        border-radius: 3px;
        font-size: 11px;
        cursor: pointer;
    }

    .restart-btn:hover {
        opacity: 0.9;
    }

    .settings-btn {
        margin-left: auto;
        background: none;
        border: none;
        color: var(--fg-dim);
        font-size: 16px;
        cursor: pointer;
        padding: 0 4px;
        line-height: 1;
    }

    .settings-btn:hover {
        color: var(--fg);
    }

    .settings-panel {
        display: flex;
        align-items: center;
        gap: 16px;
        padding: 6px 12px;
        background: var(--bg-dark);
        border-top: 1px solid var(--border);
    }

    .settings-label {
        display: flex;
        align-items: center;
        gap: 6px;
        color: var(--fg-dim);
        font-size: 11px;
        white-space: nowrap;
    }

    .settings-select {
        background: var(--bg-input, var(--bg));
        color: var(--fg);
        border: 1px solid var(--border);
        border-radius: 3px;
        font-size: 11px;
        padding: 2px 4px;
        max-width: 200px;
    }

    .gain-slider {
        width: 80px;
        accent-color: var(--accent);
    }
</style>
