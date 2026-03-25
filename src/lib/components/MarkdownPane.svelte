<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { invoke } from '@tauri-apps/api/core';
    import { appState } from '$lib/stores/app.svelte';
    import { Marked } from 'marked';
    import hljs from 'highlight.js';
    import 'highlight.js/styles/tokyo-night-dark.css';
    import { createAnnotationManager } from '$lib/annotations.svelte';
    import { createMarkedAnnotationExtension, getAnnotationOrder } from '$lib/marked-annotations';
    import AnnotationSidebar from './AnnotationSidebar.svelte';
    import AnnotationForm from './AnnotationForm.svelte';
    import { MessageSquareText } from 'lucide-svelte';
    import { platform } from '@tauri-apps/plugin-os';
    import { daemonFilepath } from '$lib/utils/markdown-path';

    // Mermaid is heavy — load it lazily only when a diagram needs rendering.
    let _mermaid: typeof import('mermaid').default | null = null;
    async function getMermaid() {
        if (!_mermaid) {
            const { default: m } = await import('mermaid');
            m.initialize({ startOnLoad: false, theme: 'dark', securityLevel: 'loose' });
            _mermaid = m;
        }
        return _mermaid;
    }

    let { filename } = $props<{ filename: string }>();
    let content = $state('');
    
    const manager = createAnnotationManager();

    // The extension depends on the current annotations
    let html = $derived.by(() => {
        if (!content) return '';
        // Create a scoped instance for this render
        const markedInstance = new Marked();
        
        // Base setup
        markedInstance.use({
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

        const ext = createMarkedAnnotationExtension(() => manager.annotations);
        return markedInstance.use(ext).parse(content) as string;
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
    let annotateError = $state<string | null>(null);

    // Selection state for annotation form
    let selectionInfo = $state<{
        quote: string;
        top: number;
        bottom: number;
        codeRange?: string;
        codeblockHandle?: string;
        codeTextPrefix?: string;
    } | null>(null);

    function getActiveSessionInfo() {
        const activeSession = appState.activeSession;
        if (!activeSession) return null;

        // filename is already "projectId/relative/path.md" — pass it directly to the daemon.
        return { hostAddress: activeSession.hostAddress, filepath: daemonFilepath(filename) };
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

    /**
     * Get the cumulative string index of a (node, offset) position within a container,
     * counting through all text nodes in tree order.
     */
    function getNodeOffsetInElement(node: Node, nodeOffset: number, container: HTMLElement): number {
        let total = 0;
        const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
        let current: Node | null;
        while ((current = walker.nextNode())) {
            if (current === node) return total + nodeOffset;
            total += (current as Text).textContent?.length ?? 0;
        }
        return total + nodeOffset;
    }

    /**
     * Convert a string index into a 0-indexed {line, char} position,
     * counting Unicode codepoints (not UTF-16 code units).
     */
    function textOffsetToLineChar(text: string, stringOffset: number): { line: number; char: number } {
        let line = 0, char = 0, strIdx = 0;
        for (const cp of text) {
            if (strIdx >= stringOffset) break;
            if (cp === '\n') { line++; char = 0; }
            else { char++; }
            strIdx += cp.length; // 1 for BMP, 2 for surrogate pairs
        }
        return { line, char };
    }

    /**
     * Find an existing `[comments](#handle)` anchor element right after a <pre> block.
     * Checks the next 3 sibling elements.
     */
    function findCodeblockHandle(preEl: HTMLElement): string | null {
        let next = preEl.nextElementSibling;
        for (let i = 0; i < 3 && next; i++) {
            const a = next.matches('a') ? (next as HTMLAnchorElement)
                    : (next.querySelector('a') as HTMLAnchorElement | null);
            if (a?.textContent?.trim() === 'comments') {
                const href = a.getAttribute('href') ?? '';
                if (href.startsWith('#')) return href.slice(1);
            }
            next = next.nextElementSibling;
        }
        return null;
    }

    /**
     * If the selection is entirely within a <pre><code> element, returns code block info.
     */
    function getCodeBlockInfo(sel: Selection): {
        codeEl: HTMLElement;
        preEl: HTMLElement;
        codeTextPrefix: string;
        codeRange: string;
        existingHandle: string | null;
    } | null {
        if (!sel.rangeCount) return null;
        const range = sel.getRangeAt(0);
        const ancestor = range.commonAncestorContainer;
        const codeEl = (ancestor.nodeType === Node.ELEMENT_NODE
            ? (ancestor as Element)
            : ancestor.parentElement
        )?.closest('pre code') as HTMLElement | null;
        if (!codeEl) return null;

        const preEl = codeEl.closest('pre') as HTMLElement | null;
        if (!preEl) return null;

        const codeText = codeEl.textContent ?? '';
        const startOffset = getNodeOffsetInElement(range.startContainer, range.startOffset, codeEl);
        const endOffset = getNodeOffsetInElement(range.endContainer, range.endOffset, codeEl);
        const start = textOffsetToLineChar(codeText, startOffset);
        const end   = textOffsetToLineChar(codeText, endOffset);

        const codeTextPrefix = codeText.slice(0, 100);
        const codeRange = `${start.line}:${start.char}->${end.line}:${end.char}`;
        const existingHandle = findCodeblockHandle(preEl);

        return { codeEl, preEl, codeTextPrefix, codeRange, existingHandle };
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
                if (sel?.isCollapsed) selectionInfo = null;
                return;
            }

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
            const top = rect.top - paneRect.top;
            const bottom = rect.bottom - paneRect.top;

            // Detect if selection is within a fenced code block
            const codeInfo = getCodeBlockInfo(sel);
            if (codeInfo) {
                const handle = codeInfo.existingHandle ?? randomId(6);
                selectionInfo = {
                    quote,
                    top,
                    bottom,
                    codeRange: codeInfo.codeRange,
                    codeblockHandle: handle,
                    codeTextPrefix: codeInfo.codeTextPrefix,
                };
            } else {
                selectionInfo = { quote, top, bottom };
            }

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
        annotateError = null;

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
                    author,
                    body,
                    codeRange: selectionInfo.codeRange ?? null,
                    codeblockHandle: selectionInfo.codeblockHandle ?? null,
                    codeTextPrefix: selectionInfo.codeTextPrefix ?? null,
                });

                // Reload content to show the new inline (quote)[#annid] marker
                await loadContent();

                selectionInfo = null;
                window.getSelection()?.removeAllRanges();
                if (!sidebarOpen && manager.annotations.length > 0) {
                    sidebarOpen = true;
                }
            } catch (e) {
                console.error("Failed to annotate file:", e);
                annotateError = "Failed to create annotation. Please try again.";
            }
        }
    }

    function handleScrollTo(annId: string) {
        activeAnnotationId = annId;
        if (!paneEl) return;
        const el = paneEl.querySelector(`[data-ann-id="${annId}"]`);
        if (el) {
            el.scrollIntoView({ behavior: 'smooth', block: 'center' });
            el.classList.add('ann-flash');
            setTimeout(() => el.classList.remove('ann-flash'), 1500);
        }
    }

    /**
     * Highlight a quote string within a DOM subtree by wrapping matching text
     * nodes in ann-highlight spans. Handles quotes that span multiple text nodes
     * (e.g., across bold/italic elements). Returns true if the quote was found.
     *
     * Set `searchInCode` to true when the container IS a code element (so we
     * don't skip pre/code descendants — only skip existing highlights).
     */
    function highlightQuoteInContainer(
        container: HTMLElement,
        quote: string,
        annId: string,
        searchInCode = false,
    ): boolean {
        // Skip if already highlighted
        if (container.querySelector(`[data-ann-id="${annId}"]`)) return true;

        // Collect all text nodes
        const walker = document.createTreeWalker(
            container,
            NodeFilter.SHOW_TEXT,
            {
                acceptNode(node: Node) {
                    const parent = (node as Text).parentElement;
                    if (!parent) return NodeFilter.FILTER_REJECT;
                    // In prose: skip code/pre blocks and existing highlights
                    // In code: only skip existing highlights
                    if (!searchInCode && parent.closest('code, pre, [data-ann-id]')) return NodeFilter.FILTER_REJECT;
                    if (searchInCode && parent.closest('[data-ann-id]')) return NodeFilter.FILTER_REJECT;
                    return NodeFilter.FILTER_ACCEPT;
                }
            }
        );

        const textNodes: Text[] = [];
        let node: Node | null;
        while ((node = walker.nextNode())) {
            textNodes.push(node as Text);
        }

        // Concatenate to find the quote
        const fullText = textNodes.map(n => n.textContent ?? '').join('');
        const idx = fullText.indexOf(quote);
        if (idx === -1) return false;

        // Wrap matching portions of text nodes in highlight spans
        let offset = 0;
        const quoteStart = idx;
        const quoteEnd = idx + quote.length;

        for (const textNode of textNodes) {
            const len = textNode.textContent?.length ?? 0;
            const nodeStart = offset;
            const nodeEnd = offset + len;
            offset += len;

            if (nodeEnd <= quoteStart || nodeStart >= quoteEnd) continue;

            const localStart = Math.max(0, quoteStart - nodeStart);
            const localEnd = Math.min(len, quoteEnd - nodeStart);
            const text = textNode.textContent ?? '';

            const span = document.createElement('span');
            span.className = 'ann-highlight';
            span.dataset.annId = annId;
            span.textContent = text.slice(localStart, localEnd);

            const parent = textNode.parentNode!;
            if (localStart > 0) {
                parent.insertBefore(document.createTextNode(text.slice(0, localStart)), textNode);
            }
            parent.insertBefore(span, textNode);
            if (localEnd < len) {
                parent.insertBefore(document.createTextNode(text.slice(localEnd)), textNode);
            }
            parent.removeChild(textNode);
        }

        return true;
    }

    /**
     * Highlight a code block annotation by searching within its associated <code> element.
     * Finds the [comments](#handle) anchor, then walks back to the <pre><code> block.
     */
    function highlightCodeBlockQuote(ann: { id: string; quote: string; codeblock_handle?: string }): boolean {
        if (!paneEl || !ann.codeblock_handle || !ann.quote) return false;

        // Find the [comments](#handle) anchor rendered in the DOM
        const anchor = paneEl.querySelector(
            `a.code-ann-anchor[data-codeblock-handle="${ann.codeblock_handle}"]`
        ) as HTMLElement | null;
        if (!anchor) return false;

        // The <pre> should be an earlier sibling of the anchor (or its parent's sibling)
        const anchorParent = anchor.parentElement;
        if (!anchorParent) return false;

        // Walk backwards through siblings to find the <pre>
        let sib: Element | null = anchor.previousElementSibling;
        if (!sib && anchorParent !== paneEl) {
            sib = anchorParent.previousElementSibling;
        }
        while (sib && !sib.matches('pre')) {
            sib = sib.previousElementSibling;
        }
        const preEl = sib as HTMLElement | null;
        if (!preEl?.matches('pre')) return false;

        const codeEl = preEl.querySelector('code') as HTMLElement | null;
        if (!codeEl) return false;

        return highlightQuoteInContainer(codeEl, ann.quote, ann.id, true);
    }

    /**
     * Apply DOM-based highlights for all annotations that have a quote.
     * Regular annotations: search in prose. Code annotations: search in code elements.
     * Called after both HTML and annotations are ready.
     */
    function applyAnnotationHighlights() {
        if (!paneEl) return;
        for (const ann of manager.annotations) {
            if (!ann.quote) continue;
            if (ann.codeblock_handle) {
                highlightCodeBlockQuote(ann);
            } else {
                highlightQuoteInContainer(paneEl, ann.quote, ann.id);
            }
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

            // 1. Code block annotation anchors [comments](#handle)
            if (link.classList.contains('code-ann-anchor')) {
                e.preventDefault();
                const handle = link.dataset.codeblockHandle;
                if (handle) {
                    const ann = manager.annotations.find(a => a.codeblock_handle === handle);
                    if (ann) {
                        activeAnnotationId = ann.id;
                        if (!sidebarOpen) sidebarOpen = true;
                    }
                }
                return;
            }

            // 2. Handle other fragment links (TOC / legacy annotation links)
            if (href.startsWith('#')) {
                e.preventDefault();
                const id = href.slice(1);

                // Is it a regular annotation?
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

    // Re-apply annotation highlights whenever annotations update (e.g., new arrival via WS)
    $effect(() => {
        const _anns = manager.annotations;
        if (paneEl && _anns.length > 0) {
            requestAnimationFrame(() => applyAnnotationHighlights());
        }
    });

    // Extract TOC and handle Mermaid after HTML renders
    $effect(() => {
        const _html = html;
        if (paneEl && _html) {
            requestAnimationFrame(async () => {
                if (!paneEl) return;

                // Apply DOM-based annotation highlights (quote search)
                applyAnnotationHighlights();

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

                // Run Mermaid diagrams (lazy-loaded on first diagram)
                try {
                    const mermaidNodes = paneEl.querySelectorAll('.mermaid');
                    if (mermaidNodes.length > 0) {
                        const mermaid = await getMermaid();
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

        {#if annotateError}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
                class="annotate-error"
                role="alert"
                onclick={() => { annotateError = null; }}
            >{annotateError}</div>
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
    .annotate-error {
        position: absolute;
        bottom: 1rem;
        left: 50%;
        transform: translateX(-50%);
        background: var(--pico-del-color, #d33);
        color: #fff;
        font-size: 0.8rem;
        padding: 0.5rem 1rem;
        border-radius: 6px;
        max-width: 80%;
        text-align: center;
        cursor: pointer;
        z-index: 50;
    }

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
        background-color: rgba(234, 179, 8, 0.18);
        border-bottom: 1.5px solid rgba(234, 179, 8, 0.55);
        padding: 0.1em 0.25em;
        border-radius: 3px;
        cursor: pointer;
    }

    .markdown-pane :global(.ann-highlight:hover) {
        background-color: rgba(234, 179, 8, 0.32);
    }

    .markdown-pane :global(.ann-highlight.ann-flash) {
        background-color: rgba(234, 179, 8, 0.5);
        transition: background-color 1.5s ease-out;
    }

    .markdown-pane :global(.ann-marker) {
        display: none;
    }

    /* Code block annotation anchors: [comments](#handle) */
    .markdown-pane :global(.code-ann-anchor) {
        display: inline-block;
        font-size: 0.72rem;
        font-family: 'InconsolataGoNerdFontMono', monospace;
        color: var(--fg-dim);
        text-decoration: none;
        opacity: 0.75;
        cursor: pointer;
        padding: 0.1em 0.45em;
        border: 1px solid var(--pico-border-color);
        border-radius: 4px;
        margin: 0 0.25em;
        vertical-align: middle;
        transition: opacity 0.15s, color 0.15s;
    }

    .markdown-pane :global(.code-ann-anchor:hover) {
        opacity: 1;
        color: var(--accent);
        border-color: var(--accent);
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
