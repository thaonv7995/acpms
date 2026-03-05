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
});
