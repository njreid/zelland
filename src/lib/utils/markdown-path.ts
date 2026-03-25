/**
 * Resolve the daemon-relative path for a file entry from openMarkdownFiles.
 *
 * Filenames in openMarkdownFiles are of the form "projectId/relative/path.md",
 * which maps directly to the daemon's /api/v1/fs/read?path=... parameter
 * (relative to the daemon's projects_path).
 *
 * Previously, MarkdownPane derived the project prefix from project.root_path and
 * prepended it, which doubled the project directory when filenames already carry
 * the prefix — resulting in paths like "myproject/myproject/README.md".
 */
export function daemonFilepath(filename: string): string {
    return filename.startsWith('./') ? filename.slice(2) : filename;
}
