import { describe, it, expect, beforeEach } from 'vitest';
import { highlightAnnotations } from './marked-annotations';
import type { Ann } from './annotations.svelte';

describe('marked-annotations highlight', () => {
    let container: HTMLElement;

    beforeEach(() => {
        container = document.createElement('div');
        document.body.appendChild(container);
    });

    it('should wrap text from a marker', () => {
        container.innerHTML = '<span class="ann-marker" data-ann-id="ann1"></span>Hello world';
        const anns: Ann[] = [{
            id: 'ann1',
            selector: { quote: 'Hello', prefix: '', suffix: '' },
            thread: []
        }];

        highlightAnnotations(container, anns);

        const highlight = container.querySelector('.ann-highlight');
        expect(highlight).toBeTruthy();
        expect(highlight?.textContent).toBe('Hello');
        expect(highlight?.getAttribute('data-ann-id')).toBe('ann1');
    });

    it('should find text by prefix and quote fallback', () => {
        container.innerHTML = '<p>The quick brown fox jumps over the lazy dog</p>';
        const anns: Ann[] = [{
            id: 'ann2',
            selector: { quote: 'fox', prefix: 'brown ', suffix: ' jumps' },
            thread: []
        }];

        highlightAnnotations(container, anns);

        const highlight = container.querySelector('.ann-highlight');
        expect(highlight).toBeTruthy();
        expect(highlight?.textContent).toBe('fox');
    });

    it('should find text by quote only if prefix/suffix fail', () => {
        container.innerHTML = '<p>Some unique text here</p>';
        const anns: Ann[] = [{
            id: 'ann3',
            selector: { quote: 'unique', prefix: 'wrong', suffix: 'wrong' },
            thread: []
        }];

        highlightAnnotations(container, anns);

        const highlight = container.querySelector('.ann-highlight');
        expect(highlight).toBeTruthy();
        expect(highlight?.textContent).toBe('unique');
    });

    it('should clear previous highlights before re-highlighting', () => {
        container.innerHTML = '<span class="ann-highlight" data-ann-id="old">Old text</span>';
        const anns: Ann[] = [];

        highlightAnnotations(container, anns);

        expect(container.querySelector('.ann-highlight')).toBeNull();
        expect(container.textContent).toBe('Old text');
    });

    it('should handle multi-node quotes (marker case)', () => {
        container.innerHTML = '<span class="ann-marker" data-ann-id="m1"></span>Hello <b>world</b> here';
        const anns: Ann[] = [{
            id: 'm1',
            selector: { quote: 'Hello world', prefix: '', suffix: '' },
            thread: []
        }];

        highlightAnnotations(container, anns);

        const highlights = container.querySelectorAll('.ann-highlight');
        // 'Hello ' is in one text node, 'world' is in another (inside <b>)
        expect(highlights.length).toBeGreaterThan(0);
        expect(container.textContent).toContain('Hello world');
    });
});
