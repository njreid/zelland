import { describe, it, expect } from 'vitest';
import { daemonFilepath } from './markdown-path';

describe('daemonFilepath', () => {
    it('passes through a project-prefixed root file unchanged', () => {
        // "myproject/README.md" should reach the daemon as-is so it resolves to
        // <projects_path>/myproject/README.md — NOT "myproject/myproject/README.md".
        expect(daemonFilepath('myproject/README.md')).toBe('myproject/README.md');
    });

    it('passes through nested docs paths unchanged', () => {
        expect(daemonFilepath('myproject/docs/guide.md')).toBe('myproject/docs/guide.md');
        expect(daemonFilepath('myproject/docs/sub/ref.md')).toBe('myproject/docs/sub/ref.md');
    });

    it('strips a leading ./ if present', () => {
        expect(daemonFilepath('./README.md')).toBe('README.md');
    });

    it('does NOT use root_path to construct the path', () => {
        // Regression: the old code did
        //   projectName = root_path.split('/').pop()    // "myproject"
        //   filepath = `${projectName}/${filename}`     // "myproject/myproject/README.md"
        // This test documents that the filename already includes the project prefix.
        const filename = 'myproject/README.md';
        const root_path = '/home/user/projects/myproject';
        const projectName = root_path.split('/').filter(Boolean).pop()!;
        const broken = `${projectName}/${filename}`;    // old (wrong) behaviour
        const fixed  = daemonFilepath(filename);        // new (correct) behaviour

        expect(broken).toBe('myproject/myproject/README.md'); // documents the bug
        expect(fixed).toBe('myproject/README.md');            // documents the fix
        expect(fixed).not.toBe(broken);
    });
});
