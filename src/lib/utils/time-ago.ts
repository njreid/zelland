/**
 * Returns a human-readable relative time string for an ISO 8601 timestamp.
 * Examples: "just now", "5m ago", "2h ago", "3d ago"
 */
export function timeAgo(isoString: string): string {
    const ms = Date.now() - new Date(isoString).getTime();
    const s = Math.floor(ms / 1000);
    if (s < 60) return 'just now';
    const m = Math.floor(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.floor(m / 60);
    if (h < 24) return `${h}h ago`;
    const d = Math.floor(h / 24);
    return `${d}d ago`;
}
