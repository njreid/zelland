<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { appState } from '$lib/stores/app.svelte';
    import { marked } from 'marked';
    import hljs from 'highlight.js';
    import mermaid from 'mermaid';
    import 'highlight.js/styles/tokyo-night-dark.css';
    import { createAnnotationManager } from '$lib/annotations.svelte';
    import { createMarkedAnnotationExtension, getAnnotationOrder } from '$lib/marked-annotations';
    import AnnotationSidebar from './AnnotationSidebar.svelte';
    import AnnotationForm from './AnnotationForm.svelte';
    import { MessageSquareText } from 'lucide-svelte';
    import { platform } from '@tauri-apps/plugin-os';

    // Base marked setup (code highlighting, mermaid)
    marked.use({
        renderer: {
            code(token) {
                const { lang, text } = token;
                if (lang === 'mermaid') {
                    return `<pre class="mermaid">${text}</pre>`;
                }
                const language = (lang && hljs.getLanguage(lang)) ? lang : 'plaintext';
                const highlighted = hljs.highlight(text, { language }).value;
                return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
            }
        }
    });

    let { filename } = $props<{ filename: string }>();
    let content = $state('');
    
    const manager = createAnnotationManager();

    // The extension depends on the current annotations
    let html = $derived.by(() => {
        if (!content) return '';
        // Create a one-off extension for this render to ensure it has latest annotations
        const ext = createMarkedAnnotationExtension(() => manager.annotations);
        return marked.use(ext).parse(content) as string;
    });

    let paneEl: HTMLDivElement | undefined = $state();
    let isDesktop = $state(false);
    let annotationOrder = $state<string[]>([]);

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

    let tocItems = $state<{ id: string; text: string; level: number }[]>([]);
    let activeTocId = $state<string | null>(null);

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

    function randomId(len: number): string {
        const chars = 'abcdefghijklmnopqrstuvwxyz0123456789';
        let result = '';
        const arr = new Uint8Array(len);
        crypto.getRandomValues(arr);
        for (const b of arr) {
            result += chars[b % chars.length];
        }
        return result;
    }

    async function handleCreate(body: string) {
        if (!selectionInfo) return;
        const author = getAuthor();
        const annId = randomId(5);

        const info = getActiveSessionInfo();
        if (info) {
            try {
                const daemonUrl = `http://${info.hostAddress}:8083`;
                const path = info.filepath;
                await invoke("daemon_annotate_file", {
                    url: daemonUrl,
                    path,
                    annId,
                    quote: selectionInfo.quote,
                    prefix: selectionInfo.prefix,
                    author,
                    body
                });
                
                // Reload content to show the new anchor
                await loadContent();
            } catch (e) {
                console.error("Failed to annotate source file:", e);
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
                
                // Is it an annotation?
                if (manager.annotations.some(a => a.id === id)) {
                    activeAnnotationId = id;
                    if (!sidebarOpen) sidebarOpen = true;
                    return;
                }

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

    function updateActiveToc() {
        if (!paneEl || tocItems.length === 0) return;
        const scrollTop = paneEl.scrollTop;
        let best: string | null = tocItems[0]?.id ?? null;
        for (const item of tocItems) {
            const el = paneEl.querySelector(`#${CSS.escape(item.id)}`);
            if (el && (el as HTMLElement).offsetTop <= scrollTop + 80) {
                best = item.id;
            }
        }
        activeTocId = best;
    }

    onMount(async () => {
        mermaid.initialize({
            startOnLoad: false,
            theme: 'dark',
            securityLevel: 'loose',
        });
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

    // Extract TOC and handle Mermaid after HTML renders
    $effect(() => {
        const _html = html;
        if (paneEl && _html) {
            requestAnimationFrame(async () => {
                if (!paneEl) return;
                
                // Track annotation order in DOM
                annotationOrder = getAnnotationOrder(paneEl);

                const headings = Array.from(
                    paneEl.querySelectorAll('h1[id], h2[id], h3[id], h4[id], h5[id], h6[id]')
                );
                tocItems = headings
                    .map(h => ({
                        id: h.id,
                        text: h.textContent?.trim() ?? '',
                        level: parseInt(h.tagName[1], 10)
                    }))
                    .filter(h => h.text);
                updateActiveToc();

                // Run Mermaid diagrams
                try {
                    const mermaidNodes = paneEl.querySelectorAll('.mermaid');
                    if (mermaidNodes.length > 0) {
                        await mermaid.run({
                            nodes: Array.from(mermaidNodes) as HTMLElement[]
                        });
                    }
                } catch (e) {
                    console.error('Mermaid error:', e);
                }
            });
        } else if (!_html) {
            tocItems = [];
            activeTocId = null;
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
            onscroll={updateActiveToc}
            style="font-size: {appState.markdownFontSize}px; font-weight: {appState.markdownFontWeight}; --code-font-size: {appState.markdownFontSize}px; --code-font-weight: {appState.markdownFontWeight};"
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

    {#if tocItems.length > 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <nav class="toc-panel" aria-label="Table of contents">
            <p class="toc-title">Contents</p>
            {#each tocItems as item}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <a
                    class="toc-item"
                    class:toc-active={activeTocId === item.id}
                    style="padding-left: calc(0.4rem + {item.level - 1} * 0.65rem)"
                    href="#{item.id}"
                    onclick={(e) => {
                        e.preventDefault();
                        const el = paneEl?.querySelector(`#${CSS.escape(item.id)}`);
                        if (el) {
                            el.scrollIntoView({ behavior: 'smooth', block: 'start' });
                            activeTocId = item.id;
                        }
                    }}
                >{item.text}</a>
            {/each}
        </nav>
    {/if}

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
        padding: 1.5rem 2rem;
        min-height: 0;
        /* Center content at a comfortable reading width */
        max-width: 760px;
        width: 100%;
        margin-left: auto;
        margin-right: auto;
        box-sizing: border-box;
    }

    .markdown-pane :global(pre) {
        background-color: #1a1b26;
        border: 1px solid var(--pico-border-color);
        border-radius: 8px;
        margin: 1.25rem 0;
        overflow: hidden;
    }

    .markdown-pane :global(pre code) {
        padding: 1rem !important;
        display: block;
        font-family: 'InconsolataGoNerdFontMono', monospace;
        font-size: var(--code-font-size, 13px);
        font-weight: var(--code-font-weight, 400);
        line-height: 1.5;
    }

    .markdown-pane :global(p code),
    .markdown-pane :global(li code) {
        background-color: var(--bg-input);
        padding: 0.15rem 0.3rem;
        border-radius: 4px;
        font-family: 'InconsolataGoNerdFontMono', monospace;
        font-size: var(--code-font-size, 13px);
        font-weight: var(--code-font-weight, 400);
    }

    .markdown-pane :global(.mermaid) {
        background-color: transparent;
        margin: 1.5rem 0;
        display: flex;
        justify-content: center;
    }

    .markdown-pane :global(.mermaid svg) {
        max-width: 100%;
        height: auto;
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

    /* TOC floating navigator — positioned just right of the centered content.
       The content max-width is 760px, so its right edge sits at calc(50% + 380px).
       We start the TOC 10px further right, giving calc(50% + 390px).
       At viewports < 1180px the TOC overflows the pane's clipping boundary and
       disappears naturally; the media query makes it display:none before that. */
    .toc-panel {
        position: absolute;
        left: calc(50% + 390px);
        top: 0;
        width: 200px;
        max-height: 100%;
        overflow-y: auto;
        overflow-x: hidden;
        padding: 1.25rem 0.5rem 1.25rem 0.75rem;
        display: none;
        scrollbar-width: thin;
        scrollbar-color: var(--pico-border-color) transparent;
    }

    @media (min-width: 1180px) {
        .toc-panel {
            display: block;
        }
    }

    .toc-title {
        font-size: 0.68rem;
        text-transform: uppercase;
        letter-spacing: 0.1em;
        color: var(--fg-dim);
        margin: 0 0 0.5rem 0;
        font-weight: 600;
    }

    .toc-item {
        display: block;
        font-size: 0.76rem;
        line-height: 1.5;
        color: var(--fg-dim);
        text-decoration: none;
        padding-top: 0.15rem;
        padding-bottom: 0.15rem;
        padding-right: 0.5rem;
        border-left: 2px solid transparent;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        transition: color 0.15s, border-color 0.15s;
    }

    .toc-item:hover {
        color: var(--pico-color);
    }

    .toc-item.toc-active {
        color: var(--accent);
        border-left-color: var(--accent);
    }
</style>
