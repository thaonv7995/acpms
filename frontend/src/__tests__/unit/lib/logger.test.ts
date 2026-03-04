import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  __restoreConsoleForTests,
  __setLoggingEnabledForTests,
  isLoggingEnabled,
  logger,
  silenceConsoleInProduction,
} from '../../../lib/logger';

describe('logger', () => {
  afterEach(() => {
    __setLoggingEnabledForTests(undefined);
    __restoreConsoleForTests();
    vi.restoreAllMocks();
  });

  it('emits logs when logging is enabled', () => {
    __setLoggingEnabledForTests(true);
    const logSpy = vi.spyOn(console, 'log').mockImplementation(() => undefined);

    logger.log('hello', 123);

    expect(isLoggingEnabled()).toBe(true);
    expect(logSpy).toHaveBeenCalledTimes(1);
    expect(logSpy).toHaveBeenCalledWith('hello', 123);
  });

  it('does not emit logs when logging is disabled', () => {
    __setLoggingEnabledForTests(false);
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => undefined);

    logger.error('hidden');

    expect(isLoggingEnabled()).toBe(false);
    expect(errorSpy).not.toHaveBeenCalled();
  });

  it('silences global console methods when logging is disabled', () => {
    __setLoggingEnabledForTests(false);
    const originalWarn = console.warn;

    silenceConsoleInProduction();

    expect(console.warn).not.toBe(originalWarn);
    expect(console.warn('message')).toBeUndefined();
  });

  it('keeps global console methods unchanged when logging is enabled', () => {
    __setLoggingEnabledForTests(true);
    const originalInfo = console.info;

    silenceConsoleInProduction();

    expect(console.info).toBe(originalInfo);
  });
});
