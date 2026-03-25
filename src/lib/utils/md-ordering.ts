const PRIORITY_ORDER = ['README.md', 'DESIGN.md', 'PLAN.md'];

/**
 * Returns a sort key for a markdown file path.
 * Priority files (README, DESIGN, PLAN) come first (0, 1, 2),
 * docs/** files come last (1000), everything else is 100.
 * Paths may be prefixed with a project id segment (e.g. "myproject/README.md").
 */
export function mdFileOrder(path: string): number {
    // Strip a project-id prefix (first path segment) when present so that
    // e.g. "myproject/README.md" and "README.md" both rank correctly.
    const segments = path.split('/');
    const name = segments.length > 1 ? segments.slice(1).join('/') : path;
    const pi = PRIORITY_ORDER.indexOf(name);
    if (pi !== -1) return pi;
    // Match docs/ whether it appears at the top level (bare path) or after a project prefix.
    if (name.startsWith('docs/') || path.startsWith('docs/')) return 1000;
    return 100;
}

/**
 * Insert a file path into a sorted list, deduplicating if already present.
 * Order: priority files → other root .md → docs/** (alphabetical within each tier).
 */
export function insertOrdered(files: string[], newFile: string): string[] {
    if (files.includes(newFile)) return files;
    return [...files, newFile].sort((a, b) => {
        const od = mdFileOrder(a) - mdFileOrder(b);
        return od !== 0 ? od : a.localeCompare(b);
    });
}
