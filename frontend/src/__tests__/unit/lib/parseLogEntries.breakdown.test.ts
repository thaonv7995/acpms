import { describe, expect, it } from 'vitest';
import { parseLogEntries } from '../../../lib/parseLogEntries';

describe('parseLogEntries breakdown formatting', () => {
  it('formats BREAKDOWN_TASK after sdk fragments are combined', () => {
    const rawLogs = [
      {
        id: 'l1',
        log_type: 'normalized',
        timestamp: '2026-03-05T09:29:20.000Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content: 'BREAKDOWN_TASK {"title":"Define Permission Request',
          timestamp: '2026-03-05T09:29:20.000Z',
        }),
      },
      {
        id: 'l2',
        log_type: 'normalized',
        timestamp: '2026-03-05T09:29:20.200Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content:
            ' And Denial Flow","description":"Specify when permissions are requested at runtime"',
          timestamp: '2026-03-05T09:29:20.200Z',
        }),
      },
      {
        id: 'l3',
        log_type: 'normalized',
        timestamp: '2026-03-05T09:29:20.400Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content: ',"task_type":"feature","priority":"medium","kind":"implementation"}',
          timestamp: '2026-03-05T09:29:20.400Z',
        }),
      },
    ];

    const entries = parseLogEntries(rawLogs);
    const assistant = entries.find((entry) => entry.type === 'assistant_message');
    expect(assistant).toBeDefined();
    expect(assistant?.content).toContain('Proposed Breakdown Tasks:');
    expect(assistant?.content).toContain('**Define Permission Request And Denial Flow**');
    expect(assistant?.content).toContain('Type: Feature · Priority: Medium');
    expect(assistant?.content).not.toContain('BREAKDOWN_TASK {');
  });

  it('formats escaped BREAKDOWN_TASK payloads from plain system logs', () => {
    const rawLogs = [
      {
        id: 'sys1',
        log_type: 'system',
        timestamp: '2026-03-05T09:29:21.000Z',
        content:
          'BREAKDOWN_TASK {\\"title\\":\\"Create acceptance test checklist\\",\\"description\\":\\"Write executable scenarios\\",\\"task_type\\":\\"test\\",\\"priority\\":\\"high\\",\\"kind\\":\\"implementation\\"}',
      },
    ];

    const entries = parseLogEntries(rawLogs);
    expect(entries).toHaveLength(1);
    expect(entries[0].type).toBe('assistant_message');
    expect(entries[0].content).toContain('**Create acceptance test checklist**');
    expect(entries[0].content).toContain('Type: Test · Priority: High');
    expect(entries[0].content).not.toContain('BREAKDOWN_TASK {');
  });

  it('repairs fragmented Claude summary layout into readable paragraphs', () => {
    const rawLogs = [
      {
        id: 'claude-summary-1',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:49:33.000Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content:
            "All\ntasks\ncomplete\n.\nHere\n's the\nsummary:\n\nInit\n\nTask Report\n\nPref\nlight &\nEnvironment\n\n•\n**GITLAB _PA T**: Present\n\nGit\nLab Repository\n\n•\nURL: https://gitlab.t\nha\nonv\n.online/t\nhaonv/landing\n-page-9898",
          timestamp: '2026-03-07T08:49:33.000Z',
        }),
      },
    ];

    const entries = parseLogEntries(rawLogs);
    expect(entries).toHaveLength(1);
    expect(entries[0].type).toBe('assistant_message');
    expect(entries[0].content).toContain("All tasks complete. Here's the summary:");
    expect(entries[0].content).toContain('Init Task Report');
    expect(entries[0].content).toContain('Preflight & Environment');
    expect(entries[0].content).toContain('GitLab Repository');
    expect(entries[0].content).toContain('https://gitlab.thaonv.online/thaonv/landing-page-9898');
  });

  it('rebuilds fragmented Claude markdown summary into headings and bullets', () => {
    const rawLogs = [
      {
        id: 'claude-summary-2',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:49:33.000Z',
        content: JSON.stringify({
          entry_type: { type: 'assistant_message' },
          content:
            "All\ntasks\ncomplete\n.\nHere\n's the\nsummary:\n---\n## Init\nTask Report\n###\nPref\nlight &\nEnvironment\n- **\nGITLAB\n_PA\nT**: Present\n- **GITLAB\n_URL\n**: Present (`\nhttps://gitlab.t\nha\non\nv\n.online\n`)\n### GitLab Repository\n- **Status\n**: Already\nexists\n(\nHTTP\n200)\n- **URL**: `\nhttps://gitlab.thaonv\n.online/t\nhaonv/landing\n-page-\n9\n898\n`\n- **Visibility\n**: private",
          timestamp: '2026-03-07T08:49:33.000Z',
        }),
      },
    ];

    const entries = parseLogEntries(rawLogs);
    expect(entries).toHaveLength(1);
    expect(entries[0].type).toBe('assistant_message');
    expect(entries[0].content).toContain("All tasks complete. Here's the summary:");
    expect(entries[0].content).toContain('## Init Task Report');
    expect(entries[0].content).toContain('### Preflight & Environment');
    expect(entries[0].content).toContain('- **GITLAB_PAT**: Present');
    expect(entries[0].content).toContain('- **GITLAB_URL**: Present (`https://gitlab.thaonv.online`)');
    expect(entries[0].content).toContain('### GitLab Repository');
    expect(entries[0].content).toContain('- **Status**: Already exists (HTTP 200)');
    expect(entries[0].content).toContain('https://gitlab.thaonv.online/thaonv/landing-page-9898');
  });

  it('merges and repairs fragmented thinking deltas', () => {
    const rawLogs = [
      {
        id: 'thinking-1',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:40:00.000Z',
        content: JSON.stringify({
          entry_type: { type: 'thinking' },
          content: 'Pref',
          timestamp: '2026-03-07T08:40:00.000Z',
        }),
      },
      {
        id: 'thinking-2',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:40:00.050Z',
        content: JSON.stringify({
          entry_type: { type: 'thinking' },
          content: 'light',
          timestamp: '2026-03-07T08:40:00.050Z',
        }),
      },
      {
        id: 'thinking-3',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:40:00.100Z',
        content: JSON.stringify({
          entry_type: { type: 'thinking' },
          content: '\n•\nchecks:\n-\nNo refs\nmanifest\n.json -\nno\nreferences\nto check',
          timestamp: '2026-03-07T08:40:00.100Z',
        }),
      },
      {
        id: 'thinking-4',
        log_type: 'normalized',
        timestamp: '2026-03-07T08:40:00.150Z',
        content: JSON.stringify({
          entry_type: { type: 'thinking' },
          content: '\n•\nNo .\nac\np\nms\nrefs\ndirectory',
          timestamp: '2026-03-07T08:40:00.150Z',
        }),
      },
    ];

    const entries = parseLogEntries(rawLogs);
    expect(entries).toHaveLength(1);
    expect(entries[0].type).toBe('thinking');
    expect(entries[0].content).toContain('Preflight');
    expect(entries[0].content).toContain('- checks:');
    expect(entries[0].content).toContain('No refs manifest.json - no references to check');
    expect(entries[0].content).toContain('No .acpms refs directory');
  });
});
