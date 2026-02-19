/**
 * Custom marked extension for annotation anchors: [|ID|]
 *
 * Renders zero-width marker spans in the markdown output. After rendering,
 * call highlightAnnotations() to wrap the actual quote text with styled spans.
 */

import type { MarkedExtension, TokenizerAndRendererExtension } from 'marked';
import type { Ann } from './annotations.svelte';

const annotationAnchor: TokenizerAndRendererExtension = {
	name: 'annotationAnchor',
	level: 'inline',
	start(src: string) {
		return src.indexOf('[|');
	},
	tokenizer(src: string) {
		const match = /^\[\|([A-Za-z0-9]+)\|\]/.exec(src);
		if (match) {
			return {
				type: 'annotationAnchor',
				raw: match[0],
				id: match[1]
			};
		}
		return undefined;
	},
	renderer(token) {
		return `<span class="ann-marker" data-ann-id="${(token as any).id}"></span>`;
	}
};

/**
 * Simple slugify for header IDs
 */
function slugify(text: string): string {
	return text
		.toLowerCase()
		.replace(/[^\w\s-]/g, '')
		.replace(/\s+/g, '-')
		.replace(/-+/g, '-')
		.trim();
}

const headerIds: MarkedExtension = {
	renderer: {
		heading({ text, depth }) {
			const id = slugify(text);
			return `<h${depth} id="${id}">${text}</h${depth}>\n`;
		}
	}
};

export const markedAnnotationExtension: MarkedExtension = {
	extensions: [annotationAnchor],
	renderer: headerIds.renderer
};

/**
 * Highlight annotation quotes in the rendered HTML container.
 *
 * Strategy:
 * 1. For each annotation, look for a `.ann-marker[data-ann-id="ID"]` span.
 * 2. From that marker, walk forward through text nodes and wrap
 *    `selector.quote.length` characters in a highlight span.
 * 3. Fallback: if no marker found, do a fuzzy text search using prefix/quote/suffix.
 */
export function highlightAnnotations(container: HTMLElement, anns: Ann[]) {
	// Clear previous highlights
	container.querySelectorAll('.ann-highlight').forEach((el) => {
		const parent = el.parentNode;
		if (!parent) return;
		while (el.firstChild) {
			parent.insertBefore(el.firstChild, el);
		}
		parent.removeChild(el);
	});

	for (const ann of anns) {
		if (!ann.selector.quote) continue;

		const marker = container.querySelector(`.ann-marker[data-ann-id="${ann.id}"]`);
		if (marker) {
			wrapFromMarker(marker, ann);
		} else {
			wrapByTextSearch(container, ann);
		}
	}
}

/**
 * Returns the IDs of all annotations found in the container, 
 * in the order they appear in the document.
 */
export function getAnnotationOrder(container: HTMLElement): string[] {
	return Array.from(container.querySelectorAll('.ann-highlight'))
		.map((el) => (el as HTMLElement).dataset.annId)
		.filter((id, index, self) => id && self.indexOf(id) === index) as string[];
}

function wrapFromMarker(marker: Element, ann: Ann) {
	const quote = ann.selector.quote;
	let remaining = quote.length;

	// Walk text nodes after the marker
	let node: Node | null = nextTextNode(marker);
	const ranges: { node: Text; start: number; end: number }[] = [];

	while (node && remaining > 0) {
		if (node.nodeType === Node.TEXT_NODE) {
			const text = node as Text;
			const available = text.textContent?.length ?? 0;
			const take = Math.min(remaining, available);
			ranges.push({ node: text, start: 0, end: take });
			remaining -= take;
		}
		node = nextTextNode(node);
	}

	applyRanges(ranges, ann.id);
}

function wrapByTextSearch(container: HTMLElement, ann: Ann) {
	const fullText = container.textContent ?? '';
	const quote = ann.selector.quote;
	const prefix = ann.selector.prefix;
	const suffix = ann.selector.suffix;
	let idx = -1;

	// 1. Try full match: prefix + quote + suffix
	if (prefix && suffix) {
		const needle = prefix + quote + suffix;
		const found = fullText.indexOf(needle);
		if (found >= 0) {
			idx = found + prefix.length;
		}
	}

	// 2. Try prefix + quote
	if (idx < 0 && prefix) {
		const needle = prefix + quote;
		const found = fullText.indexOf(needle);
		if (found >= 0) {
			idx = found + prefix.length;
		}
	}

	// 3. Try quote + suffix
	if (idx < 0 && suffix) {
		const needle = quote + suffix;
		idx = fullText.indexOf(needle);
	}

	// 4. Fallback to plain quote search
	if (idx < 0) {
		idx = fullText.indexOf(quote);
	}

	if (idx < 0) return;

	// Find the text nodes corresponding to this character range
	const ranges = findTextRanges(container, idx, idx + quote.length);
	applyRanges(ranges, ann.id);
}

function findTextRanges(
	container: HTMLElement,
	start: number,
	end: number
): { node: Text; start: number; end: number }[] {
	const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
	const ranges: { node: Text; start: number; end: number }[] = [];
	let offset = 0;

	while (walker.nextNode()) {
		const text = walker.currentNode as Text;
		const len = text.textContent?.length ?? 0;
		const nodeEnd = offset + len;

		if (nodeEnd > start && offset < end) {
			const rangeStart = Math.max(0, start - offset);
			const rangeEnd = Math.min(len, end - offset);
			ranges.push({ node: text, start: rangeStart, end: rangeEnd });
		}

		offset = nodeEnd;
		if (offset >= end) break;
	}

	return ranges;
}

function applyRanges(ranges: { node: Text; start: number; end: number }[], annId: string) {
	// Apply in reverse order to avoid invalidating ranges
	for (let i = ranges.length - 1; i >= 0; i--) {
		const { node, start, end } = ranges[i];
		const range = document.createRange();
		range.setStart(node, start);
		range.setEnd(node, end);

		const span = document.createElement('span');
		span.className = 'ann-highlight';
		span.dataset.annId = annId;
		range.surroundContents(span);
	}
}

function nextTextNode(node: Node): Node | null {
	let current: Node | null = node;

	// Try first child
	if (current.firstChild) return findFirstText(current.firstChild);

	// Try next sibling, or walk up
	while (current) {
		if (current.nextSibling) return findFirstText(current.nextSibling);
		current = current.parentNode;
	}
	return null;
}

function findFirstText(node: Node): Node | null {
	if (node.nodeType === Node.TEXT_NODE) return node;
	let child = node.firstChild;
	while (child) {
		const found = findFirstText(child);
		if (found) return found;
		child = child.nextSibling;
	}
	// If this node has no text children, try next sibling
	return null;
}
