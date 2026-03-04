type ConsoleMethod = 'log' | 'debug' | 'info' | 'warn' | 'error' | 'trace';

const SILENCED_METHODS: readonly ConsoleMethod[] = ['log', 'debug', 'info', 'warn', 'error', 'trace'];
const originalConsoleMethods: Partial<Record<ConsoleMethod, Console[ConsoleMethod]>> = {};
let isConsoleSilenced = false;

declare global {
  // Test-only override for deterministic logger assertions.
  // eslint-disable-next-line no-var
  var __ACPMS_FORCE_LOGGING_ENABLED__: boolean | undefined;
}

export function isLoggingEnabled(): boolean {
  const forcedValue = globalThis.__ACPMS_FORCE_LOGGING_ENABLED__;
  if (typeof forcedValue === 'boolean') return forcedValue;
  return import.meta.env.DEV;
}

function emit(method: ConsoleMethod, ...args: unknown[]): void {
  if (!isLoggingEnabled()) return;
  console[method](...args);
}

export const logger = {
  log: (...args: unknown[]) => emit('log', ...args),
  debug: (...args: unknown[]) => emit('debug', ...args),
  info: (...args: unknown[]) => emit('info', ...args),
  warn: (...args: unknown[]) => emit('warn', ...args),
  error: (...args: unknown[]) => emit('error', ...args),
  trace: (...args: unknown[]) => emit('trace', ...args),
};

export function silenceConsoleInProduction(): void {
  if (isConsoleSilenced || isLoggingEnabled()) return;

  for (const method of SILENCED_METHODS) {
    const originalMethod = console[method].bind(console);
    originalConsoleMethods[method] = originalMethod;
    console[method] = (() => undefined) as Console[typeof method];
  }

  isConsoleSilenced = true;
}

export function __setLoggingEnabledForTests(value: boolean | undefined): void {
  globalThis.__ACPMS_FORCE_LOGGING_ENABLED__ = value;
}

export function __restoreConsoleForTests(): void {
  for (const method of SILENCED_METHODS) {
    const originalMethod = originalConsoleMethods[method];
    if (originalMethod) {
      console[method] = originalMethod;
    }
  }
  isConsoleSilenced = false;
}
