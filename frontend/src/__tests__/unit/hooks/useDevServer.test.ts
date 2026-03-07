import { describe, expect, it } from 'vitest';
import {
  extractPreviewSignalFromAttemptLogs,
  extractPreviewUrlFromAttemptLogs,
  extractPreviewUrlFromText,
  isPreviewAlreadyStoppedMessage,
  isPreviewReadinessBlockingMessage,
  mapPreviewErrorMessage,
} from '../../../hooks/useDevServer';

describe('useDevServer error helpers', () => {
  it('detects readiness-blocking backend messages', () => {
    expect(
      isPreviewReadinessBlockingMessage(
        'Preview unavailable: missing Cloudflare config: cloudflare_zone_id'
      )
    ).toBe(true);
    expect(
      isPreviewReadinessBlockingMessage(
        'Preview unavailable: Docker preview runtime is disabled'
      )
    ).toBe(true);
    expect(
      isPreviewReadinessBlockingMessage(
        'Preview is disabled in project settings'
      )
    ).toBe(true);
    expect(
      isPreviewReadinessBlockingMessage(
        "Preview not supported for project type 'mobile'"
      )
    ).toBe(true);
    expect(isPreviewReadinessBlockingMessage('docker compose up failed')).toBe(
      false
    );
  });

  it('maps command resolution/package errors to actionable guidance', () => {
    expect(
      mapPreviewErrorMessage(
        "Unable to resolve preview command from package.json for project type 'web'. Tried scripts [dev, start], available scripts [lint, test]."
      )
    ).toContain('No compatible start script found for preview');

    expect(
      mapPreviewErrorMessage(
        'Failed to parse package.json for preview command resolution: expected value at line 1 column 1'
      )
    ).toContain('Cannot read package.json for preview command detection');

    expect(
      mapPreviewErrorMessage(
        "Unable to resolve preview command for project type 'api' because package.json is missing at /tmp/app and no supported non-Node entrypoint (Python/Go/Rust) was detected."
      )
    ).toContain('Cannot detect Python/Go/Rust preview entrypoint');
  });

  it('maps docker daemon/runtime startup errors', () => {
    expect(
      mapPreviewErrorMessage(
        'Failed to execute docker compose up for attempt 1: No such file or directory'
      )
    ).toContain('Docker compose command failed to execute');

    expect(
      mapPreviewErrorMessage(
        'docker compose up failed for attempt 1: Cannot connect to the Docker daemon at unix:///var/run/docker.sock. Is the docker daemon running?'
      )
    ).toContain('Cannot connect to Docker daemon');

    expect(
      mapPreviewErrorMessage(
        'Timed out after 90s waiting for dev-server and cloudflared to be running'
      )
    ).toContain('startup timed out');
  });

  it('maps non-node runtime dependency/toolchain errors', () => {
    expect(
      mapPreviewErrorMessage('ModuleNotFoundError: No module named uvicorn')
    ).toContain('Python preview dependencies are missing');

    expect(
      mapPreviewErrorMessage('/bin/sh: go: command not found')
    ).toContain('Runtime image does not include required toolchain');
  });

  it('detects and normalizes already-stopped preview message', () => {
    expect(isPreviewAlreadyStoppedMessage('Preview not found')).toBe(true);
    expect(
      mapPreviewErrorMessage('Preview not found for this attempt')
    ).toBe('Preview is already stopped for this attempt.');
  });

  it('extracts PREVIEW_TARGET from plain-text output', () => {
    expect(
      extractPreviewUrlFromText(
        'Deployment ready\nPREVIEW_TARGET:\nhttp://127.0.0.1:4321\n'
      )
    ).toBe('http://127.0.0.1:4321');

    expect(
      extractPreviewUrlFromText('Expected format: PREVIEW_TARGET: http://127.0.0.1:<port>')
    ).toBeUndefined();
  });

  it('strips trailing JSON artifacts from preview URL candidates', () => {
    expect(
      extractPreviewUrlFromText('PREVIEW_TARGET: http://127.0.0.1:8080"}')
    ).toBe('http://127.0.0.1:8080');

    expect(
      extractPreviewUrlFromText('PREVIEW_URL: https://preview.example.com"}')
    ).toBe('https://preview.example.com');
  });

  it('extracts the latest PREVIEW_TARGET from attempt logs', () => {
    expect(
      extractPreviewUrlFromAttemptLogs([
        {
          id: '1',
          attempt_id: 'attempt-1',
          log_type: 'stdout',
          content: 'PREVIEW_TARGET: http://127.0.0.1:3000',
          created_at: '2026-03-06T08:00:00Z',
        },
        {
          id: '2',
          attempt_id: 'attempt-1',
          log_type: 'stdout',
          content:
            '{"content":"Deploy summary\\nPREVIEW_TARGET: http://127.0.0.1:4321"}',
          created_at: '2026-03-06T08:01:00Z',
        },
      ])
    ).toBe('http://127.0.0.1:4321');
  });

  it('returns a stable preview signal key for the latest preview log', () => {
    expect(
      extractPreviewSignalFromAttemptLogs([
        {
          id: '1',
          attempt_id: 'attempt-1',
          log_type: 'stdout',
          content: 'PREVIEW_TARGET: http://127.0.0.1:3000',
          created_at: '2026-03-06T08:00:00Z',
        },
        {
          id: '2',
          attempt_id: 'attempt-1',
          log_type: 'stdout',
          content:
            '{"content":"Deploy summary\\nPREVIEW_TARGET: http://127.0.0.1:4321"}',
          created_at: '2026-03-06T08:01:00Z',
        },
      ])
    ).toEqual({
      url: 'http://127.0.0.1:4321',
      signalKey: '2:2026-03-06T08:01:00Z:http://127.0.0.1:4321',
    });
  });
});
