import { beforeAll, describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ProjectAssistantChat } from '@/components/project-assistant/ProjectAssistantChat';

beforeAll(() => {
  Object.defineProperty(HTMLElement.prototype, 'scrollTo', {
    configurable: true,
    writable: true,
    value: vi.fn(),
  });
});

function renderChat(messages: Array<{
  id: string;
  session_id: string;
  role: string;
  content: string;
  metadata?: { tool_calls?: Array<{ id: string; name: string; args: Record<string, unknown> }> };
  created_at: string;
}>) {
  render(
    <ProjectAssistantChat
      projectId="project-1"
      sessionId="session-1"
      messages={messages}
      onSendMessage={vi.fn().mockResolvedValue(true)}
      onRefresh={vi.fn()}
      onStartAgent={vi.fn()}
      agentActive={false}
      starting={false}
      loading={false}
      readOnly={false}
    />
  );
}

describe('ProjectAssistantChat message visibility', () => {
  it('keeps assistant response visible even when content starts with "Starting "', () => {
    renderChat([
      {
        id: 'assistant-1',
        session_id: 'session-1',
        role: 'assistant',
        content: 'Starting with a quick summary of what I found.',
        created_at: '2026-03-04T10:00:00.000Z',
      },
      {
        id: 'user-1',
        session_id: 'session-1',
        role: 'user',
        content: 'ok',
        created_at: '2026-03-04T10:00:01.000Z',
      },
    ]);

    expect(
      screen.getByText('Starting with a quick summary of what I found.')
    ).toBeTruthy();
  });

  it('hides assistant internal-only status lines after sanitization', () => {
    renderChat([
      {
        id: 'assistant-1',
        session_id: 'session-1',
        role: 'assistant',
        content: 'Preparing initial codebase inspection',
        created_at: '2026-03-04T10:00:00.000Z',
      },
    ]);

    expect(screen.queryByText('Preparing initial codebase inspection')).toBeNull();
    expect(screen.getByRole('button', { name: 'Start' })).toBeTruthy();
  });
});
