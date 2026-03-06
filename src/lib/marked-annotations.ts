/**
 * Custom marked extension for annotation handling.
 *
 * - Handles new inline format: (quoted text)[#ann_id]
 * - Strips the "## Comments" section (and its preceding "---") from rendering
 * - Intercepts legacy annotation links [text](#ID) for backwards compatibility
 */

import type { MarkedExtension } from 'marked';
import type { Ann } from './annotations.svelte';

/**
 * Extension to handle annotation markers, strip the comments section,
 * and support legacy annotation links.
 */
export function createMarkedAnnotationExtension(getAnns: () => Ann[]): MarkedExtension {
	return {
		// Inline tokenizer for (quoted text)[#ann_id] — not valid standard markdown,
		// so markdown formatters (marksman etc.) leave it untouched.
		extensions: [
			{
				name: 'annInline',
				level: 'inline' as const,
				// Hint: tokens of this type can only start with '('
				start(src: string) {
					return src.indexOf('(');
				},
				tokenizer(src: string) {
					const match = /^\(([^)\n]+)\)\[#([a-z0-9]+)\]/.exec(src);
					if (match) {
						return {
							type: 'annInline',
							raw: match[0],
							annId: match[2],
							text: match[1]
						};
					}
				},
				renderer(token: any) {
					const anns = getAnns();
					if (anns.some((a: any) => a.id === token.annId)) {
						return `<span class="ann-highlight" data-ann-id="${token.annId}">${token.text}</span>`;
					}
					// Annotation not yet loaded — render as plain text, DOM highlight handles it
					return `(${token.text})[#${token.annId}]`;
				}
			}
		],

		renderer: {
			link({ href, text }) {
				if (href.startsWith('#')) {
					const handle = href.slice(1);
					const anns = getAnns();

					// [comments](#handle) — code block annotation anchor
					if (text === 'comments') {
						const count = anns.filter(a => a.codeblock_handle === handle).length;
						const label = count === 1 ? '1 comment' : `${count} comments`;
						return `<a class="code-ann-anchor" href="${href}" data-codeblock-handle="${handle}">${label}</a>`;
					}

					// Legacy annotation links [text](#ann_id) from old-format files
					if (anns.some(a => a.id === handle)) {
						return `<span class="ann-highlight" data-ann-id="${handle}">${text}</span>`;
					}
				}
				return `<a href="${href}">${text}</a>`;
			},
			heading({ text, depth }) {
				// Strip "Comments" section header (h1 old format or h2 new format)
				if ((depth === 1 || depth === 2) && text.trim() === 'Comments') {
					return '';
				}
				const id = text.toLowerCase().replace(/[^\w\s-]/g, '').replace(/\s+/g, '-').trim();
				return `<h${depth} id="${id}">${text}</h${depth}>\n`;
			}
		},

		hooks: {
			preprocess(markdown) {
				// Strip everything from the comments section and any preceding "---" separator.
				const patterns = [
					'\n---\n\n## Comments',
					'\n---\n## Comments',
					'\n---\n\n# Comments',
					'\n---\n# Comments',
					'\n# Comments',
					'\n## Comments'
				];
				for (const pat of patterns) {
					const idx = markdown.indexOf(pat);
					if (idx !== -1) {
						return markdown.slice(0, idx);
					}
				}
				if (markdown.startsWith('## Comments') || markdown.startsWith('# Comments')) {
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
	return Array.from(container.querySelectorAll('[data-ann-id]'))
		.map((el) => (el as HTMLElement).dataset.annId)
		.filter((id, index, self) => id && self.indexOf(id) === index) as string[];
}
