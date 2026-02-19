<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { appState } from '$lib/stores/app.svelte';
    import { marked } from 'marked';
    import { createAnnotationManager } from '$lib/annotations.svelte';
    import { markedAnnotationExtension, highlightAnnotations, getAnnotationOrder } from '$lib/marked-annotations';
    import AnnotationSidebar from './AnnotationSidebar.svelte';
    import AnnotationForm from './AnnotationForm.svelte';
    import { MessageSquareText } from 'lucide-svelte';
    import { platform } from '@tauri-apps/plugin-os';

    // Register the [|ID|] annotation anchor extension once
    marked.use(markedAnnotationExtension);

    let { filename } = $props<{ filename: string }>();
    let content = $state('');
    let html = $derived(content ? marked.parse(content) : '');
    let paneEl: HTMLDivElement | undefined = $state();
    let isDesktop = $state(false);
    let annotationOrder = $state<string[]>([]);

    const manager = createAnnotationManager();

    // Derived sorted annotations based on document order
    let orderedAnnotations = $derived.by(() => {
        const anns = manager.annotations;
        if (annotationOrder.length === 0) return anns;
        
        const sorted = [...anns].sort((a, b) => {
            const idxA = annotationOrder.indexOf(a.id);
            const idxB = annotationOrder.indexOf(b.id);
            if (idxA === -1 && idxB === -1) return 0;
            if (idxA === -1) return 1;
            if (idxB === -1) return -1;
            return idxA - idxB;
        });
        return sorted;
    });

    let sidebarOpen = $state(false);
    let activeAnnotationId = $state<string | null>(null);
    let isSelecting = false;

    // Selection state for annotation form
    let selectionInfo = $state<{ 
        quote: string; 
        prefix: string; 
        suffix: string;
        top: number;
        bottom: number;
    } | null>(null);

    function getActiveSessionInfo() {
        const activeSession = appState.activeSession;
        const project = appState.activeProject;
        if (!activeSession || !project?.root_path) return null;

        const projectName = project.root_path.split('/').filter(Boolean).pop() ?? '';
        const filepath = `${projectName}/${filename}`;

        return { hostAddress: activeSession.hostAddress, filepath, rootPath: project.root_path };
    }

    function getAuthor(): string {
        return appState.activeSession?.username ?? 'anon';
    }

    async function loadContent() {
        content = '';
        appState.setFileLoaded(filename, false);

        const info = getActiveSessionInfo();
        if (!info) return;

        try {
            const daemonUrl = `http://${info.hostAddress}:8083`;
            const path = info.filepath;

            const res = await invoke<string>("daemon_read_file", { url: daemonUrl, path });

            if (res.includes("no such file") || res.includes("Status 404")) {
                console.warn(`File not found on daemon: ${filename}`);
                appState.setFileLoaded(filename, false);
            } else {
                content = res;
                appState.setFileLoaded(filename, true);
            }
        } catch (e) {
            console.error(`Failed to load ${filename}:`, e);
            appState.setFileLoaded(filename, false);
        }
    }

    function connectAnnotations() {
        const info = getActiveSessionInfo();
        if (info) {
            manager.connect(info.hostAddress, info.filepath);
        } else {
            manager.disconnect();
        }
    }

    function handleMouseDown() {
        isSelecting = true;
    }

    // Handle text selection for creating annotations
    function handleMouseUp() {
        isSelecting = false;
        
        // Wait a tiny bit for the selection to stabilize
        setTimeout(() => {
            const sel = window.getSelection();
            if (!sel || sel.isCollapsed || !sel.rangeCount || !paneEl) {
                // If it's a simple click (isCollapsed), clear selection info
                if (sel?.isCollapsed) selectionInfo = null;
                return;
            }

            // Only handle selections within our pane
            const range = sel.getRangeAt(0);
            if (!paneEl.contains(range.commonAncestorContainer)) {
                selectionInfo = null;
                return;
            }

            const quote = sel.toString().trim();
            if (!quote || quote.length < 2) {
                selectionInfo = null;
                return;
            }

            const rect = range.getBoundingClientRect();
            const paneRect = paneEl.getBoundingClientRect();
            
            // Calculate relative positions
            const top = rect.top - paneRect.top;
            const bottom = rect.bottom - paneRect.top;

            // Extract prefix/suffix from the container's text
            const fullText = paneEl.textContent ?? '';
            const idx = fullText.indexOf(quote);
            const prefix = idx >= 0 ? fullText.slice(Math.max(0, idx - 30), idx) : '';
            const suffix = idx >= 0 ? fullText.slice(idx + quote.length, idx + quote.length + 30) : '';
            
            selectionInfo = { quote, prefix, suffix, top, bottom };
            
            if (isDesktop && !sidebarOpen) {
                sidebarOpen = true;
            }
        }, 50);
    }

    async function handleCreate(body: string) {
        if (!selectionInfo) return;
        const author = getAuthor();
        const annId = manager.createAnnotation(
            selectionInfo.quote,
            selectionInfo.prefix,
            selectionInfo.suffix,
            author,
            body || undefined
        );

        if (annId) {
            // Also mutate the source file on the server to include the marker
            const info = getActiveSessionInfo();
            if (info) {
                try {
                    const daemonUrl = `http://${info.hostAddress}:8083`;
                    const path = info.filepath;
                    await invoke("daemon_mutate_file", {
                        url: daemonUrl,
                        path,
                        annId,
                        quote: selectionInfo.quote,
                        prefix: selectionInfo.prefix,
                        suffix: selectionInfo.suffix
                    });
                } catch (e) {
                    console.error("Failed to mutate source file:", e);
                }
            }
        }

        selectionInfo = null;
        window.getSelection()?.removeAllRanges();
        if (!sidebarOpen && manager.annotations.length > 0) {
            sidebarOpen = true;
        }
    }

    function handleScrollTo(annId: string) {
        activeAnnotationId = annId;
        if (!paneEl) return;
        const el = paneEl.querySelector(`.ann-highlight[data-ann-id="${annId}"]`);
        if (el) {
            el.scrollIntoView({ behavior: 'smooth', block: 'center' });
            el.classList.add('ann-flash');
            setTimeout(() => el.classList.remove('ann-flash'), 1500);
        }
    }

    function handleAddComment(annId: string, body: string) {
        manager.addComment(annId, getAuthor(), body);
    }

    function handleDelete(annId: string) {
        manager.deleteAnnotation(annId);
        if (activeAnnotationId === annId) activeAnnotationId = null;
    }

    function handlePaneClick(e: MouseEvent) {
        const target = e.target as HTMLElement;
        
        // Handle links
        const link = target.closest('a') as HTMLAnchorElement | null;
        if (link && link.getAttribute('href')) {
            const href = link.getAttribute('href')!;
            
            // 1. Handle fragment links (TOC)
            if (href.startsWith('#')) {
                e.preventDefault();
                const id = href.slice(1);
                const targetEl = paneEl?.querySelector(`[id="${id}"]`);
                if (targetEl) {
                    targetEl.scrollIntoView({ behavior: 'smooth', block: 'start' });
                }
                return;
            }

            // 2. Check if it's a relative markdown link
            if (href.endsWith('.md') && !href.startsWith('http') && !href.startsWith('//')) {
                e.preventDefault();
                appState.openMarkdownFile(href);
                return;
            }
        }

        const highlight = target.closest('.ann-highlight') as HTMLElement | null;
        if (highlight?.dataset.annId) {
            activeAnnotationId = highlight.dataset.annId;
            if (!sidebarOpen) sidebarOpen = true;
        }
    }

    function goBack() {
        appState.scrollToPane(0); // Back to terminal
    }

    function cancelSelection() {
        selectionInfo = null;
        window.getSelection()?.removeAllRanges();
    }

    onMount(async () => {
        loadContent();
        connectAnnotations();
        
        const osPlatform = await platform();
        isDesktop = osPlatform === 'linux' || osPlatform === 'macos' || osPlatform === 'windows';
    });

    onDestroy(() => {
        manager.disconnect();
    });

    // Reload content when view update triggers fire
    $effect(() => {
        if (appState.viewUpdateTrigger && appState.viewUpdateTrigger[filename] !== undefined) {
            loadContent();
        }
    });

    // Reconnect when active session changes or becomes connected
    $effect(() => {
        const session = appState.activeSession;
        if (session?.status === 'connected') {
            loadContent();
            connectAnnotations();
        }
    });

    // Highlight annotations after HTML renders
    $effect(() => {
        const _html = html;
        const _anns = manager.annotations;
        const _size = appState.markdownFontSize;
        const _weight = appState.markdownFontWeight;
        console.log(`MarkdownPane: highlighting/styling updated (size=${_size}, weight=${_weight})`);
        
        if (paneEl && _html && _anns.length > 0) {
            requestAnimationFrame(() => {
                if (paneEl) {
                    highlightAnnotations(paneEl, _anns);
                    annotationOrder = getAnnotationOrder(paneEl);
                }
            });
        }
    });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="mdpane-root">
    <div class="mdpane-content">
        <div
            class="markdown-pane container"
            id="pane-{filename}"
            bind:this={paneEl}
            onmousedown={handleMouseDown}
            onmouseup={handleMouseUp}
            onclick={handlePaneClick}
            style="font-size: {appState.markdownFontSize}px; font-weight: {appState.markdownFontWeight};"
        >
            {#if html}
                {@html html}
            {:else}
                <div class="mdpane-empty">
                    <div class="text-center">
                        <p><small>No {filename} found for active session.</small></p>
                        <button class="outline secondary btn-sm" onclick={goBack}>Back to Terminal</button>
                    </div>
                </div>
            {/if}
        </div>

        {#if selectionInfo && !isDesktop}
            <AnnotationForm
                quote={selectionInfo.quote}
                top={selectionInfo.bottom + 20}
                mode="floating"
                onCreate={handleCreate}
                onCancel={cancelSelection}
            />
        {/if}
    </div>

    {#if sidebarOpen}
        <AnnotationSidebar
            annotations={orderedAnnotations}
            {activeAnnotationId}
            pendingAnnotation={isDesktop ? selectionInfo : null}
            onScrollTo={handleScrollTo}
            onAddComment={handleAddComment}
            onCreate={handleCreate}
            onCancel={cancelSelection}
            onDelete={handleDelete}
            onClose={() => { sidebarOpen = false; }}
        />
    {/if}

    {#if orderedAnnotations.length > 0 && !sidebarOpen}
        <button
            class="ann-toggle-btn"
            onclick={() => { sidebarOpen = true; }}
            aria-label="Show annotations"
        >
            <MessageSquareText size={14} />
            <span>{orderedAnnotations.length}</span>
        </button>
    {/if}
</div>

<style>
    .mdpane-root {
        height: 100%;
        position: relative;
        display: flex;
    }

    .mdpane-content {
        flex: 1;
        position: relative;
        overflow: hidden;
        display: flex;
        flex-direction: column;
        min-width: 0;
    }

    .markdown-pane {
        flex: 1;
        overflow-y: auto;
        padding: 1rem;
        min-height: 0;
    }

    .mdpane-empty {
        display: flex;
        justify-content: center;
        align-items: center;
        height: 100%;
        color: var(--fg-dim);
    }

    .markdown-pane :global(.ann-highlight) {
        text-decoration: underline;
        text-decoration-color: #3b82f6;
        text-underline-offset: 2px;
        background-color: rgba(59, 130, 246, 0.1);
        cursor: pointer;
        border-radius: 2px;
    }

    .markdown-pane :global(.ann-highlight:hover) {
        background-color: rgba(59, 130, 246, 0.2);
    }

    .markdown-pane :global(.ann-highlight.ann-flash) {
        background-color: rgba(59, 130, 246, 0.35);
        transition: background-color 1.5s ease-out;
    }

    .markdown-pane :global(.ann-marker) {
        display: none;
    }

    .ann-toggle-btn {
        position: absolute;
        top: 0.5rem;
        right: 0.5rem;
        display: flex;
        align-items: center;
        gap: 0.25rem;
        padding: 0.25rem 0.5rem;
        font-size: 0.75rem;
        background: var(--bg-input);
        border: 1px solid var(--pico-border-color);
        border-radius: 6px;
        color: var(--accent);
        cursor: pointer;
        z-index: 10;
        opacity: 0.85;
    }

    .ann-toggle-btn:hover {
        opacity: 1;
        background: var(--bg-darker);
    }
</style>
