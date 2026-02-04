<script lang="ts">
    let { title, url, fileType } = $props();
</script>

<div class="flex flex-col h-full bg-[#1a1b26] p-4 overflow-auto">
    <div class="flex items-center justify-between mb-4 border-b border-[#313244] pb-2">
        <h2 class="text-[#7aa2f7] font-bold">{title}</h2>
        <span class="text-xs text-[#565f89] uppercase tracking-wider">{fileType}</span>
    </div>

    <div class="flex-1 flex items-center justify-center">
        {#if fileType === 'IMAGE'}
            <img src={url} alt={title} class="max-w-full max-h-full object-contain shadow-lg" />
        {:else if fileType === 'MARKDOWN'}
            <div class="prose prose-invert max-w-none w-full">
                <!-- Fallback for now, in real app we'd use a markdown parser -->
                <iframe src={url} title="Markdown Content" class="w-full h-[80vh] border-none bg-white rounded" />
            </div>
        {:else}
            <div class="text-[#f7768e]">Unsupported file type: {fileType}</div>
        {/if}
    </div>
</div>
