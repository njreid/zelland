<script lang="ts">
    import type { Ann } from '$lib/annotations.svelte';
    import { MessageSquare, Trash2, X } from 'lucide-svelte';
    import AnnotationForm from './AnnotationForm.svelte';

    let {
        annotations,
        activeAnnotationId = null,
        pendingAnnotation = null,
        onScrollTo,
        onAddComment,
        onCreate,
        onCancel,
        onDelete,
        onClose
    } = $props<{
        annotations: Ann[];
        activeAnnotationId?: string | null;
        pendingAnnotation?: { quote: string; top: number } | null;
        onScrollTo: (annId: string) => void;
        onAddComment: (annId: string, body: string) => void;
        onCreate?: (body: string) => void;
        onCancel?: () => void;
        onDelete: (annId: string) => void;
        onClose: () => void;
    }>();

    let sidebarEl: HTMLElement | undefined = $state();
    let replyingTo = $state<string | null>(null);
    let replyBody = $state('');

    // Determine where to insert the pending annotation in the sorted list
    // (Assuming annotations are already sorted by document position/id)
    let insertionIndex = $derived.by(() => {
        if (!pendingAnnotation) return -1;
        // In a real implementation, we'd compare character offsets.
        // For now, we'll approximate based on vertical position if available,
        // or just append if offsets aren't tracked in the Ann type yet.
        return annotations.length; 
    });

    // Auto-scroll to the pending form
    $effect(() => {
        if (pendingAnnotation && sidebarEl) {
            const form = sidebarEl.querySelector('.ann-form-container');
            if (form) {
                form.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
            }
        }
    });

    function submitReply(annId: string) {
        if (!replyBody.trim()) return;
        onAddComment(annId, replyBody.trim());
        replyBody = '';
        replyingTo = null;
    }

    function handleReplyKeydown(e: KeyboardEvent, annId: string) {
        if (e.key === 'Enter') {
            // In sidebar, plain Enter submits for speed, unlike the main Markdown area
            e.preventDefault();
            submitReply(annId);
        } else if (e.key === 'Escape') {
            e.preventDefault();
            replyingTo = null;
            replyBody = '';
        }
    }

    // Auto-focus reply textarea
    $effect(() => {
        if (replyingTo && sidebarEl) {
            const textarea = sidebarEl.querySelector('.ann-reply-form textarea') as HTMLTextAreaElement | null;
            textarea?.focus();
        }
    });

    function formatDate(iso: string): string {
        try {
            const d = new Date(iso);
            return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
        } catch {
            return iso;
        }
    }

    function truncate(s: string, max: number): string {
        return s.length > max ? s.slice(0, max) + '\u2026' : s;
    }
</script>

<aside class="ann-sidebar" bind:this={sidebarEl}>
    <div class="ann-sidebar-header">
        <span>Annotations <small>({annotations.length})</small></span>
        <button class="icon-btn" onclick={onClose} aria-label="Close sidebar">
            <X size={16} />
        </button>
    </div>

    {#if annotations.length === 0 && !pendingAnnotation}
        <div class="ann-empty">
            <small>No annotations yet.<br/>Select text to annotate.</small>
        </div>
    {:else}
        <div class="ann-list">
            {#each annotations as ann (ann.id)}
                <div
                    class="ann-card"
                    class:active={activeAnnotationId === ann.id}
                >
                    <button class="ann-quote-btn" onclick={() => onScrollTo(ann.id)}>
                        {ann.quote ? truncate(ann.quote, 80) : `Annotation ${ann.id}`}
                    </button>

                    {#each ann.thread as comment, i}
                        {#if comment.body}
                            <div class="ann-comment">
                                <div class="ann-comment-meta">
                                    <strong>{comment.author || 'anon'}</strong>
                                    <small>{formatDate(comment.timestamp)}</small>
                                </div>
                                <p>{comment.body}</p>
                            </div>
                        {/if}
                    {/each}

                    <div class="ann-card-actions">
                        {#if replyingTo === ann.id}
                            <div class="ann-reply-form">
                                <textarea
                                    bind:value={replyBody}
                                    placeholder="Write a comment&hellip;"
                                    rows="2"
                                    onkeydown={(e) => handleReplyKeydown(e, ann.id)}
                                ></textarea>
                                <div class="ann-reply-btns">
                                    <button class="outline secondary btn-sm" onclick={() => { replyingTo = null; replyBody = ''; }}>
                                        Cancel
                                    </button>
                                    <button class="btn-sm" onclick={() => submitReply(ann.id)}>
                                        Reply
                                    </button>
                                </div>
                            </div>
                        {:else}
                            <button class="icon-btn" onclick={() => { replyingTo = ann.id; }} aria-label="Reply">
                                <MessageSquare size={14} />
                            </button>
                            <button class="icon-btn danger" onclick={() => onDelete(ann.id)} aria-label="Delete">
                                <Trash2 size={14} />
                            </button>
                        {/if}
                    </div>
                </div>
            {/each}

            {#if pendingAnnotation && onCreate && onCancel}
                <div class="ann-form-container">
                    <AnnotationForm
                        quote={pendingAnnotation.quote}
                        mode="sidebar"
                        {onCreate}
                        {onCancel}
                    />
                </div>
            {/if}
        </div>
    {/if}
</aside>

<style>
    .ann-sidebar {
        position: absolute;
        right: 0;
        top: 0;
        bottom: 0;
        width: 280px;
        overflow-y: auto;
        background: var(--bg-darker);
        border-left: 1px solid var(--pico-border-color);
        display: flex;
        flex-direction: column;
        z-index: 15;
    }

    .ann-sidebar-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0.5rem 0.75rem;
        border-bottom: 1px solid var(--pico-border-color);
        font-weight: 600;
        font-size: 0.85rem;
    }

    .ann-empty {
        padding: 2rem 1rem;
        text-align: center;
        color: var(--fg-dim);
    }

    .ann-list {
        flex: 1;
        overflow-y: auto;
    }

    .ann-card {
        border-bottom: 1px solid var(--pico-border-color);
        padding: 0.5rem 0.75rem;
    }

    .ann-card.active {
        background: var(--accent-a08);
        border-left: 2px solid var(--accent);
    }

    .ann-form-container {
        border-bottom: 1px solid var(--pico-border-color);
        background: var(--accent-a04);
        position: relative;
    }

    .ann-quote-btn {
        all: unset;
        cursor: pointer;
        display: block;
        width: 100%;
        font-size: 0.8rem;
        font-style: italic;
        color: var(--accent);
        margin-bottom: 0.25rem;
        line-height: 1.3;
        text-align: left;
    }

    .ann-quote-btn:hover {
        text-decoration: underline;
    }

    .ann-comment {
        padding: 0.25rem 0;
        font-size: 0.8rem;
    }

    .ann-comment-meta {
        display: flex;
        align-items: baseline;
        gap: 0.5rem;
        margin-bottom: 0.1rem;
    }

    .ann-comment-meta strong {
        font-size: 0.75rem;
    }

    .ann-comment-meta small {
        color: var(--fg-dim);
        font-size: 0.7rem;
    }

    .ann-comment p {
        margin: 0;
        font-size: 0.8rem;
        line-height: 1.4;
    }

    .ann-card-actions {
        display: flex;
        align-items: center;
        gap: 0.25rem;
        margin-top: 0.25rem;
    }

    .ann-reply-form {
        width: 100%;
    }

    .ann-reply-form textarea {
        width: 100%;
        font-size: 0.8rem;
        padding: 0.35rem;
        margin-bottom: 0.25rem;
    }

    .ann-reply-btns {
        display: flex;
        gap: 0.25rem;
        justify-content: flex-end;
    }

    .icon-btn {
        all: unset;
        cursor: pointer;
        padding: 0.2rem;
        border-radius: 4px;
        color: var(--fg-dim);
        display: inline-flex;
        align-items: center;
    }

    .icon-btn:hover {
        color: var(--pico-color);
        background: rgba(255, 255, 255, 0.05);
    }

    .icon-btn.danger:hover {
        color: var(--error);
    }

    .btn-sm {
        padding: 0.2rem 0.5rem;
        font-size: 0.75rem;
    }

    @media (max-width: 500px) {
        .ann-sidebar {
            width: 100%;
        }
    }
</style>
