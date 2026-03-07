import { describe, expect, it } from 'vitest';
import { formatShellCommandForDisplay } from '../../../lib/commandDisplay';

describe('formatShellCommandForDisplay', () => {
  it('strips zsh shell wrappers', () => {
    expect(
      formatShellCommandForDisplay(
        "/bin/zsh -lc 'pwd && ls -la && rg --files .acpms || true'"
      )
    ).toBe('pwd && ls -la && rg --files .acpms || true');
  });

  it('strips bash shell wrappers', () => {
    expect(
      formatShellCommandForDisplay('/bin/bash -lc "npm test"')
    ).toBe('npm test');
  });

  it('strips env and leading cd boilerplate', () => {
    expect(
      formatShellCommandForDisplay(
        'env FOO=bar BAR="baz qux" /bin/sh -lc \'cd /tmp/project && npm run build\''
      )
    ).toBe('npm run build');
  });

  it('keeps plain commands unchanged', () => {
    expect(formatShellCommandForDisplay('cargo check -p acpms-server')).toBe(
      'cargo check -p acpms-server'
    );
  });
});
