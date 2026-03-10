import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, vi, afterEach } from 'vitest';
import { PreviewPanel } from '@/components/preview/PreviewPanel';

describe('PreviewPanel', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('removes the dedicated edit button and edits the preview URL inline on double click', async () => {
    render(
      <PreviewPanel
        devServerUrl="https://preview.example.com/current"
        status="running"
        onStart={vi.fn()}
        onStop={vi.fn()}
        onDismiss={vi.fn()}
        onRestart={vi.fn()}
      />
    );

    expect(
      screen.queryByRole('button', { name: /edit preview url/i })
    ).toBeNull();

    fireEvent.doubleClick(
      screen.getByRole('button', { name: /preview url/i })
    );

    const urlInput = screen.getByRole('textbox', { name: /edit preview url/i });
    expect((urlInput as HTMLInputElement).value).toBe(
      'https://preview.example.com/current'
    );

    fireEvent.change(urlInput, {
      target: { value: 'https://preview.example.com/updated' },
    });
    fireEvent.keyDown(urlInput, { key: 'Enter' });

    await waitFor(() => {
      expect(screen.getByTitle('Dev Server Preview').getAttribute('src')).toBe(
        'https://preview.example.com/updated'
      );
    });
  });
});
