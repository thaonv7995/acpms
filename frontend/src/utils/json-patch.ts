import { logger } from '@/lib/logger';
// RFC 6902 JSON Patch implementation
// https://tools.ietf.org/html/rfc6902

export type JSONPatchOperation =
  | { op: 'add'; path: string; value: any }
  | { op: 'remove'; path: string }
  | { op: 'replace'; path: string; value: any }
  | { op: 'move'; from: string; path: string }
  | { op: 'copy'; from: string; path: string }
  | { op: 'test'; path: string; value: any };

export interface JSONPatchError {
  message: string;
  operation: JSONPatchOperation;
  path: string;
}

/**
 * Parse JSON Pointer path into array of segments
 */
function parsePath(path: string): string[] {
  if (path === '') return [];
  if (!path.startsWith('/')) {
    throw new Error(`Invalid JSON Pointer path: ${path}`);
  }
  return path
    .slice(1)
    .split('/')
    .map((segment) => segment.replace(/~1/g, '/').replace(/~0/g, '~'));
}

/**
 * Get value at JSON Pointer path
 */
function getValue(target: any, path: string): any {
  const segments = parsePath(path);
  let current = target;

  for (const segment of segments) {
    if (current === null || current === undefined) {
      throw new Error(`Cannot get value at path ${path}`);
    }
    current = current[segment];
  }

  return current;
}

/**
 * Set value at JSON Pointer path
 */
function setValue(target: any, path: string, value: any): void {
  const segments = parsePath(path);
  if (segments.length === 0) {
    throw new Error('Cannot replace root');
  }

  let current = target;
  for (let i = 0; i < segments.length - 1; i++) {
    const segment = segments[i];
    if (!(segment in current)) {
      // Auto-create intermediate objects/arrays
      const nextSegment = segments[i + 1];
      current[segment] = /^\d+$/.test(nextSegment) ? [] : {};
    }
    current = current[segment];
  }

  const lastSegment = segments[segments.length - 1];

  // Handle array append with '-'
  if (Array.isArray(current) && lastSegment === '-') {
    current.push(value);
  } else {
    current[lastSegment] = value;
  }
}

/**
 * Remove value at JSON Pointer path
 */
function removeValue(target: any, path: string): void {
  const segments = parsePath(path);
  if (segments.length === 0) {
    throw new Error('Cannot remove root');
  }

  let current = target;
  for (let i = 0; i < segments.length - 1; i++) {
    current = current[segments[i]];
    if (current === null || current === undefined) {
      throw new Error(`Cannot remove at path ${path}`);
    }
  }

  const lastSegment = segments[segments.length - 1];
  if (Array.isArray(current)) {
    const index = parseInt(lastSegment, 10);
    if (isNaN(index) || index < 0 || index >= current.length) {
      throw new Error(`Invalid array index at path ${path}`);
    }
    current.splice(index, 1);
  } else {
    delete current[lastSegment];
  }
}

/**
 * Validate a JSON Patch operation
 */
export function validatePatch(operation: JSONPatchOperation): boolean {
  if (!operation || typeof operation !== 'object') return false;

  const validOps = ['add', 'remove', 'replace', 'move', 'copy', 'test'];
  if (!validOps.includes(operation.op)) return false;

  if (!('path' in operation) || typeof operation.path !== 'string') return false;

  if (['move', 'copy'].includes(operation.op)) {
    if (!('from' in operation) || typeof operation.from !== 'string') return false;
  }

  if (['add', 'replace', 'test'].includes(operation.op)) {
    if (!('value' in operation)) return false;
  }

  return true;
}

/**
 * Apply a single JSON Patch operation to target
 */
export function applyPatch(
  target: any,
  operation: JSONPatchOperation
): { success: boolean; error?: JSONPatchError } {
  try {
    if (!validatePatch(operation)) {
      return {
        success: false,
        error: {
          message: 'Invalid patch operation',
          operation,
          path: operation.path || '',
        },
      };
    }

    switch (operation.op) {
      case 'add':
        setValue(target, operation.path, operation.value);
        break;

      case 'remove':
        removeValue(target, operation.path);
        break;

      case 'replace':
        removeValue(target, operation.path);
        setValue(target, operation.path, operation.value);
        break;

      case 'move': {
        const value = getValue(target, operation.from);
        removeValue(target, operation.from);
        setValue(target, operation.path, value);
        break;
      }

      case 'copy': {
        const value = getValue(target, operation.from);
        setValue(target, operation.path, value);
        break;
      }

      case 'test': {
        const value = getValue(target, operation.path);
        if (JSON.stringify(value) !== JSON.stringify(operation.value)) {
          throw new Error(
            `Test failed: value at ${operation.path} does not match expected`
          );
        }
        break;
      }

      default: {
        const unknownOp = operation as any;
        return {
          success: false,
          error: {
            message: `Unknown operation: ${unknownOp.op}`,
            operation,
            path: unknownOp.path || '',
          },
        };
      }
    }

    return { success: true };
  } catch (error) {
    return {
      success: false,
      error: {
        message: error instanceof Error ? error.message : 'Unknown error',
        operation,
        path: operation.path,
      },
    };
  }
}

/**
 * Apply an array of JSON Patch operations to target
 * Stops on first error and returns all results
 */
export function applyPatches(
  target: any,
  operations: JSONPatchOperation[]
): {
  success: boolean;
  appliedCount: number;
  errors: JSONPatchError[];
} {
  const errors: JSONPatchError[] = [];
  let appliedCount = 0;

  for (const operation of operations) {
    const result = applyPatch(target, operation);
    if (result.success) {
      appliedCount++;
    } else {
      if (result.error) {
        errors.push(result.error);
        logger.warn('JSON Patch error:', result.error);
      }
      // Continue applying patches even if one fails
    }
  }

  return {
    success: errors.length === 0,
    appliedCount,
    errors,
  };
}

/**
 * Deep clone an object for immutable patch application
 */
export function cloneDeep<T>(obj: T): T {
  return JSON.parse(JSON.stringify(obj));
}

/**
 * Apply patches immutably (returns new object)
 */
export function applyPatchesImmutable<T>(
  target: T,
  operations: JSONPatchOperation[]
): { result: T; success: boolean; appliedCount: number; errors: JSONPatchError[] } {
  const cloned = cloneDeep(target);
  const patchResult = applyPatches(cloned, operations);

  return {
    result: cloned,
    ...patchResult,
  };
}
