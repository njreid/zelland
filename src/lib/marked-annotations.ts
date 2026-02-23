/**
 * Custom marked extension for annotation anchors: [text](#ID)
 */

import type { MarkedExtension } from 'marked';
import type { Ann } from './annotations.svelte';

/**
 * Extension to strip the "# Comments" section and intercept annotation links
 */
export function createMarkedAnnotationExtension(getAnns: () => Ann[]): MarkedExtension {
	return {
		renderer: {
			link({ href, text }) {
				// Intercept annotation links (e.g., #k8f2a)
				if (href.startsWith('#')) {
					const id = href.slice(1);
					const anns = getAnns();
					if (anns.some(a => a.id === id)) {
						return `<span class="ann-highlight" data-ann-id="${id}">${text}</span>`;
					}
				}
				// Default link rendering
				return `<a href="${href}">${text}</a>`;
			},
			heading({ text, depth, tokens }) {
				// Strip the "# Comments" section
				if (depth === 1 && text.trim() === 'Comments') {
					return '';
				}
				
				// Standard slugify for other headers
				const id = text.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').trim();
				return `<h${depth} id="${id}">${text}</h${depth}>\n`;
			},
			list(token) {
				// If we're inside the Comments section, we might want to hide the list too.
				// But since we already return empty for the H1, 
				// we need a way to track if we're "after" the Comments header.
				// For now, let's keep it simple. The reifier adds it at the end.
				return `<ul${token.ordered ? ' start="' + token.start + '"' : ''}>${this.parser.parse(token.items)}</ul>`;
			}
		},
		hooks: {
			preprocess(markdown) {
				// Cut off everything from "# Comments" onwards
				const splitPoint = markdown.indexOf('\n# Comments');
				if (splitPoint !== -1) {
					return markdown.slice(0, splitPoint);
				}
				if (markdown.startsWith('# Comments')) {
					return '';
				}
				return markdown;
			}
		}
	};
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
