<script lang="ts">
    import { ChevronUp, ChevronDown, ChevronLeft, ChevronRight, Move } from 'lucide-svelte';
    import { SPECIAL_KEYS } from '$lib/utils/key-mapper';

    let { sendKey } = $props();

    let isExpanded = $state(false);
    let startX = 0;
    let startY = 0;
    let isDragging = false;
    let buttonRef: HTMLButtonElement | undefined = $state();

    function handleTouchStart(e: TouchEvent) {
        startX = e.touches[0].clientX;
        startY = e.touches[0].clientY;
        isDragging = false;
    }

    function handleTouchMove(e: TouchEvent) {
        if (!startX || !startY) return;
        
        const currentX = e.touches[0].clientX;
        const currentY = e.touches[0].clientY;
        
        const diffX = startX - currentX;
        const diffY = startY - currentY;
        
        // Threshold to consider it a drag
        if (Math.abs(diffX) > 10 || Math.abs(diffY) > 10) {
            isDragging = true;
        }
    }

    function handleTouchEnd(e: TouchEvent) {
        if (!startX || !startY) return;
        
        const endX = e.changedTouches[0].clientX;
        const endY = e.changedTouches[0].clientY;
        
        const diffX = startX - endX;
        const diffY = startY - endY;
        
        if (isDragging) {
            // Determine direction
            if (Math.abs(diffX) > Math.abs(diffY)) {
                // Horizontal
                if (diffX > 0) {
                    sendKey(SPECIAL_KEYS.LEFT);
                } else {
                    sendKey(SPECIAL_KEYS.RIGHT);
                }
            } else {
                // Vertical
                if (diffY > 0) {
                    sendKey(SPECIAL_KEYS.UP);
                } else {
                    sendKey(SPECIAL_KEYS.DOWN);
                }
            }
        } else {
            // Tap - Toggle expand
            isExpanded = !isExpanded;
        }
        
        startX = 0;
        startY = 0;
        isDragging = false;
    }
    
    // Mouse fallback for desktop testing
    function handleMouseDown(e: MouseEvent) {
        startX = e.clientX;
        startY = e.clientY;
        isDragging = false;
        
        const moveHandler = (moveEvent: MouseEvent) => {
             if (Math.abs(moveEvent.clientX - startX) > 5 || Math.abs(moveEvent.clientY - startY) > 5) {
                isDragging = true;
             }
        };

        const upHandler = (upEvent: MouseEvent) => {
            const diffX = startX - upEvent.clientX;
            const diffY = startY - upEvent.clientY;
            
            if (isDragging) {
                 if (Math.abs(diffX) > Math.abs(diffY)) {
                    if (diffX > 0) sendKey(SPECIAL_KEYS.LEFT);
                    else sendKey(SPECIAL_KEYS.RIGHT);
                } else {
                    if (diffY > 0) sendKey(SPECIAL_KEYS.UP);
                    else sendKey(SPECIAL_KEYS.DOWN);
                }
            } else {
                isExpanded = !isExpanded;
            }
            
            window.removeEventListener('mousemove', moveHandler);
            window.removeEventListener('mouseup', upHandler);
        };
        
        window.addEventListener('mousemove', moveHandler);
        window.addEventListener('mouseup', upHandler);
    }
    
    // Close when clicking outside
    function handleWindowClick(e: MouseEvent) {
        if (isExpanded && buttonRef && !buttonRef.contains(e.target as Node) && !(e.target as Element).closest('.arrow-popup')) {
            isExpanded = false;
        }
    }
</script>

<svelte:window onclick={handleWindowClick} />

<div class="arrow-pad-container">
    {#if isExpanded}
        <div class="arrow-popup">
            <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.UP)}><ChevronUp size={16} /></button>
            <div class="mid-row">
                <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.LEFT)}><ChevronLeft size={16} /></button>
                <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.DOWN)}><ChevronDown size={16} /></button>
                <button class="outline contrast icon-btn key-unit" onclick={() => sendKey(SPECIAL_KEYS.RIGHT)}><ChevronRight size={16} /></button>
            </div>
        </div>
    {/if}

    <button 
        bind:this={buttonRef}
        class="outline contrast icon-btn key-unit drag-pad"
        ontouchstart={handleTouchStart}
        ontouchmove={handleTouchMove}
        ontouchend={handleTouchEnd}
        onmousedown={handleMouseDown}
    >
        <Move size={18} />
    </button>
</div>

<style>
    .arrow-pad-container {
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
    }

    .drag-pad {
        touch-action: none; /* Important for preventing scroll while dragging */
    }

    .arrow-popup {
        position: absolute;
        bottom: 120%; /* Position above */
        left: 50%;
        transform: translateX(-50%);
        background-color: var(--bg-keyboard);
        border: 1px solid var(--pico-border-color);
        border-radius: 4px;
        padding: 4px;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 4px;
        box-shadow: 0 4px 10px rgba(0,0,0,0.3);
        z-index: 200;
    }

    .mid-row {
        display: flex;
        gap: 4px;
    }

    .key-unit {
        width: 2.2rem;
        height: 2.2rem;
        min-width: 2.2rem;
        padding: 0 !important;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 0.75rem;
        margin-bottom: 0 !important;
        line-height: 1;
        border-width: 1px;
    }
    
    .icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
    }
</style>