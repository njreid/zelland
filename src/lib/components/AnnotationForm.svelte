<script lang="ts">
    import { X } from 'lucide-svelte';

    let {
        quote,
        onCreate,
        onCancel,
        top = 0,
        mode = 'bottom'
    } = $props<{
        quote: string;
        onCreate: (body: string) => void;
        onCancel: () => void;
        top?: number;
        mode?: 'bottom' | 'sidebar' | 'floating';
    }>();

    let body = $state('');
    let formEl: HTMLDivElement | undefined = $state();

    function submit() {
        onCreate(body.trim());
        body = '';
    }

    function truncate(s: string, max: number): string {
        return s.length > max ? s.slice(0, max) + '\u2026' : s;
    }

    function handleKeydown(e: KeyboardEvent) {
        if (e.key === 'Enter') {
            if (e.metaKey || e.ctrlKey || mode === 'sidebar') {
                e.preventDefault();
                submit();
            }
        } else if (e.key === 'Escape') {
            e.preventDefault();
            onCancel();
        }
    }

    // Auto-focus textarea
    $effect(() => {
        if (formEl) {
            formEl.querySelector('textarea')?.focus();
        }
    });
</script>

<div 
    bind:this={formEl}
    class="ann-form {mode}" 
    style={mode === 'floating' || mode === 'bottom' ? (top ? `top: ${top}px;` : '') : ''}
>
    <div class="ann-form-header">
        <span class="ann-form-quote">&ldquo;{truncate(quote, 60)}&rdquo;</span>
        <button class="icon-btn" onclick={onCancel} aria-label="Cancel">
            <X size={14} />
        </button>
    </div>
    <textarea
        bind:value={body}
        placeholder="Add a comment&hellip;"
        rows="2"
        onkeydown={handleKeydown}
    ></textarea>
    <div class="ann-form-actions">
        <button class="outline secondary btn-sm" onclick={onCancel}>Cancel</button>
        <button class="btn-sm" onclick={submit}>Save</button>
    </div>
</div>

<style>
    .ann-form {
        background: var(--bg-input);
        border: 1px solid var(--pico-border-color);
        padding: 0.75rem;
        z-index: 100;
        box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
        border-radius: 8px;
    }

    .ann-form.bottom {
        position: absolute;
        bottom: 0;
        left: 0;
        right: 0;
        border-radius: 0;
        border-bottom: none;
        border-left: none;
        border-right: none;
    }

    .ann-form.sidebar {
        border: none;
        box-shadow: none;
        background: transparent;
        padding: 0.5rem 0.75rem;
    }

    .ann-form.floating {
        position: absolute;
        left: 1rem;
        right: 1rem;
        width: calc(100% - 2rem);
    }

    .ann-form-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        margin-bottom: 0.5rem;
    }

    .ann-form-quote {
        font-size: 0.8rem;
        font-style: italic;
        color: var(--accent);
        line-height: 1.3;
        flex: 1;
        margin-right: 0.5rem;
    }

    .ann-form textarea {
        width: 100%;
        font-size: 0.85rem;
        padding: 0.4rem;
        margin-bottom: 0.5rem;
    }

    .ann-form-actions {
        display: flex;
        gap: 0.5rem;
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
    }

    .btn-sm {
        padding: 0.25rem 0.6rem;
        font-size: 0.8rem;
    }
</style>
