import { describe, it, expect } from 'vitest';
import { mdFileOrder, insertOrdered } from './md-ordering';

describe('mdFileOrder', () => {
    it('assigns priority order 0/1/2 to README/DESIGN/PLAN', () => {
        expect(mdFileOrder('README.md')).toBe(0);
        expect(mdFileOrder('DESIGN.md')).toBe(1);
        expect(mdFileOrder('PLAN.md')).toBe(2);
    });

    it('strips project-id prefix before ranking', () => {
        expect(mdFileOrder('myproject/README.md')).toBe(0);
        expect(mdFileOrder('myproject/DESIGN.md')).toBe(1);
        expect(mdFileOrder('myproject/PLAN.md')).toBe(2);
    });

    it('assigns 100 to ordinary root .md files', () => {
        expect(mdFileOrder('NOTES.md')).toBe(100);
        expect(mdFileOrder('myproject/NOTES.md')).toBe(100);
    });

    it('assigns 1000 to docs/** files', () => {
        expect(mdFileOrder('docs/guide.md')).toBe(1000);
        expect(mdFileOrder('myproject/docs/guide.md')).toBe(1000);
        expect(mdFileOrder('myproject/docs/sub/deep.md')).toBe(1000);
    });
});

describe('insertOrdered', () => {
    it('returns original array when file already present', () => {
        const files = ['README.md'];
        const result = insertOrdered(files, 'README.md');
        expect(result).toBe(files); // same reference (no-op)
    });

    it('inserts priority files before ordinary files', () => {
        const result = insertOrdered(['NOTES.md'], 'README.md');
        expect(result).toEqual(['README.md', 'NOTES.md']);
    });

    it('maintains README < DESIGN < PLAN ordering', () => {
        let files: string[] = [];
        files = insertOrdered(files, 'PLAN.md');
        files = insertOrdered(files, 'README.md');
        files = insertOrdered(files, 'DESIGN.md');
        expect(files).toEqual(['README.md', 'DESIGN.md', 'PLAN.md']);
    });

    it('places docs/ files after ordinary files', () => {
        let files: string[] = [];
        files = insertOrdered(files, 'docs/guide.md');
        files = insertOrdered(files, 'NOTES.md');
        files = insertOrdered(files, 'README.md');
        expect(files).toEqual(['README.md', 'NOTES.md', 'docs/guide.md']);
    });

    it('sorts ordinary files alphabetically among themselves', () => {
        let files: string[] = [];
        files = insertOrdered(files, 'ZEBRA.md');
        files = insertOrdered(files, 'ALPHA.md');
        expect(files).toEqual(['ALPHA.md', 'ZEBRA.md']);
    });

    it('handles project-id-prefixed paths', () => {
        let files: string[] = [];
        files = insertOrdered(files, 'proj/NOTES.md');
        files = insertOrdered(files, 'proj/README.md');
        files = insertOrdered(files, 'proj/docs/guide.md');
        expect(files).toEqual(['proj/README.md', 'proj/NOTES.md', 'proj/docs/guide.md']);
    });
});
