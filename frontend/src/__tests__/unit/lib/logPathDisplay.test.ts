import { describe, expect, it } from 'vitest';
import {
  formatLogPathForConversation,
  formatLogPathForDisplay,
  humanizeLogText,
} from '../../../lib/logPathDisplay';

describe('formatLogPathForDisplay', () => {
  it('formats skill playbook paths with a friendly label', () => {
    expect(
      formatLogPathForDisplay(
        '/Users/thaonv/Projects/conversation-log-probe/.acpms/skills/task-preflight-check/SKILL.md'
      )
    ).toBe('task-preflight-check skill playbook');
  });

  it('shortens local repository roots to the project name', () => {
    expect(
      formatLogPathForDisplay('/Users/thaonv/Projects/Personal/Agentic-Coding')
    ).toBe('Agentic-Coding');
  });

  it('keeps repo-relative source paths when a marker is present', () => {
    expect(
      formatLogPathForDisplay(
        '/Users/thaonv/Projects/Personal/Agentic-Coding/frontend/src/components/App.tsx'
      )
    ).toBe('frontend/src/components/App.tsx');
  });

  it('drops redundant parent folders for root-level files in conversation labels', () => {
    expect(
      formatLogPathForConversation('/Users/thaonv/Projects1/landing-page-9898/README.md')
    ).toBe('README.md');
  });

  it('keeps useful repo-relative folders for nested source files in conversation labels', () => {
    expect(
      formatLogPathForConversation('/Users/thaonv/Projects1/landing-page-9898/src/App.tsx')
    ).toBe('src/App.tsx');
  });
});

describe('humanizeLogText', () => {
  it('rewrites absolute paths embedded in log sentences', () => {
    const input =
      'Repository exists at "/Users/thaonv/Projects/conversation-log-probe-1772863661", syncing latest changes from /Users/thaonv/Projects/Personal/Agentic-Coding';

    expect(humanizeLogText(input)).toBe(
      'Repository exists at "conversation-log-probe-1772863661", syncing latest changes from Agentic-Coding'
    );
  });
});
