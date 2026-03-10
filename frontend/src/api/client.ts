// Updated: 1767727008
/// <reference types="vite/client" />

// Production: same-origin so login/WS work when opened via LAN IP (e.g. http://192.168.1.4:22029)
// Development: explicit backend URL for Vite dev server
const API_BASE_URL =
  import.meta.env.VITE_API_URL ||
  (import.meta.env.PROD ? '' : 'http://localhost:3000');
// API prefix for legacy manual APIs (Orval-generated code already includes this)
export const API_PREFIX = '/api/v1';

export function getApiBaseUrl(): string {
  return API_BASE_URL;
}

/** WebSocket base URL: same-origin in production (ws(s)://current host:port), else VITE_WS_URL or ws://localhost:3000 */
export function getWsBaseUrl(): string {
  if (import.meta.env.VITE_WS_URL) return import.meta.env.VITE_WS_URL;
  if (import.meta.env.PROD && typeof window !== 'undefined') {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    return `${protocol}//${window.location.host}`;
  }
  return 'ws://localhost:3000';
}

const TOKEN_KEY = 'acpms_token';
const REFRESH_TOKEN_KEY = 'acpms_refresh_token';
const CURRENT_USER_KEY = 'acpms_current_user';
const ACCESS_TOKEN_REFRESH_BUFFER_SECONDS = 60;

type RefreshAccessTokenResult =
  | { status: 'success'; accessToken: string }
  | { status: 'missing_refresh_token' }
  | { status: 'invalid_refresh_token' }
  | { status: 'transient_error' };

let refreshInFlight: Promise<RefreshAccessTokenResult> | null = null;

export interface ApiResponse<T> {
  success: boolean;
  code: string;
  message: string;
  data: T; // Currently backend wraps Option<T> but we can assume T or T | null
  metadata?: any;
  error?: {
    details?: string;
    trace_id?: string;
  };
}

export function getAccessToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setAccessToken(token: string): void {
  localStorage.setItem(TOKEN_KEY, token);
}

export function clearAccessToken(): void {
  localStorage.removeItem(TOKEN_KEY);
}

export function getRefreshToken(): string | null {
  return localStorage.getItem(REFRESH_TOKEN_KEY);
}

export function setRefreshToken(token: string): void {
  localStorage.setItem(REFRESH_TOKEN_KEY, token);
}

export function clearRefreshToken(): void {
  localStorage.removeItem(REFRESH_TOKEN_KEY);
}

export function clearTokens(): void {
  clearAccessToken();
  clearRefreshToken();
  localStorage.removeItem(CURRENT_USER_KEY);
}

export class ApiError extends Error {
  public code: string;

  constructor(public status: number, message: string, code: string = 'UNKNOWN') {
    super(message);
    this.name = 'ApiError';
    this.code = code;
  }
}

interface RefreshTokenResponseData {
  access_token: string;
  expires_in: number;
  refresh_token?: string;
}

function isRefreshEligiblePath(path: string): boolean {
  // Auth endpoints should not trigger refresh-on-401 to avoid recursion/loop.
  return !path.startsWith('/api/v1/auth/');
}

function decodeJwtPayload(token: string): Record<string, unknown> | null {
  const [, payload] = token.split('.');
  if (!payload || typeof atob !== 'function') return null;

  try {
    const normalized = payload.replace(/-/g, '+').replace(/_/g, '/');
    const padded = normalized.padEnd(normalized.length + ((4 - normalized.length % 4) % 4), '=');
    return JSON.parse(atob(padded)) as Record<string, unknown>;
  } catch {
    return null;
  }
}

function getJwtExpiration(token: string): number | null {
  const payload = decodeJwtPayload(token);
  const exp = payload?.exp;
  return typeof exp === 'number' ? exp : null;
}

function isTokenExpiringSoon(token: string, bufferSeconds = ACCESS_TOKEN_REFRESH_BUFFER_SECONDS): boolean {
  const exp = getJwtExpiration(token);
  if (!exp) return false;

  const nowSeconds = Math.floor(Date.now() / 1000);
  return exp <= nowSeconds + bufferSeconds;
}

function shouldInvalidateSession(refreshResult: RefreshAccessTokenResult): boolean {
  return (
    refreshResult.status === 'missing_refresh_token' ||
    refreshResult.status === 'invalid_refresh_token'
  );
}

function invalidateSession(): never {
  clearTokens();
  redirectToLogin();
  throw new ApiError(401, 'Unauthorized', '4010');
}

async function refreshAccessToken(): Promise<RefreshAccessTokenResult> {
  const refreshToken = getRefreshToken();
  if (!refreshToken) return { status: 'missing_refresh_token' };

  let response: Response;
  try {
    response = await fetch(`${API_BASE_URL}/api/v1/auth/refresh`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        refresh_token: refreshToken,
      }),
    });
  } catch {
    return { status: 'transient_error' };
  }

  if (response.status === 401 || response.status === 403) {
    return { status: 'invalid_refresh_token' };
  }

  if (!response.ok) {
    return { status: 'transient_error' };
  }

  try {
    const body = (await response.json()) as ApiResponse<RefreshTokenResponseData>;
    if (!body?.success || !body?.data?.access_token) {
      return { status: 'transient_error' };
    }

    setAccessToken(body.data.access_token);
    if (body.data.refresh_token) {
      setRefreshToken(body.data.refresh_token);
    }

    return { status: 'success', accessToken: body.data.access_token };
  } catch {
    return { status: 'transient_error' };
  }
}

async function getOrRefreshAccessToken(): Promise<RefreshAccessTokenResult> {
  if (!refreshInFlight) {
    refreshInFlight = refreshAccessToken().finally(() => {
      refreshInFlight = null;
    });
  }

  return refreshInFlight;
}

function redirectToLogin(): void {
  if (typeof window !== 'undefined' && window.location.pathname !== '/login') {
    window.location.href = '/login';
  }
}

export async function authenticatedFetch(
  path: string,
  options?: RequestInit
): Promise<Response> {
  let token = getAccessToken();
  const url = `${API_BASE_URL}${path}`;

  if (token && isRefreshEligiblePath(path) && isTokenExpiringSoon(token)) {
    const refreshResult = await getOrRefreshAccessToken();
    if (refreshResult.status === 'success') {
      token = refreshResult.accessToken;
    } else if (shouldInvalidateSession(refreshResult)) {
      invalidateSession();
    }
  }

  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(options?.headers as Record<string, string>),
  };

  if (token) {
    headers['Authorization'] = `Bearer ${token}`;
  }

  const response = await fetch(url, {
    ...options,
    headers,
  });

  // Handle 401 Unauthorized
  if (response.status === 401) {
    if (isRefreshEligiblePath(path)) {
      const refreshResult = await getOrRefreshAccessToken();
      if (refreshResult.status === 'success') {
        const retryHeaders: Record<string, string> = {
          ...headers,
          Authorization: `Bearer ${refreshResult.accessToken}`,
        };

        const retryResponse = await fetch(url, {
          ...options,
          headers: retryHeaders,
        });

        if (retryResponse.status !== 401) {
          return retryResponse;
        }
      } else if (refreshResult.status === 'transient_error') {
        throw new ApiError(401, 'Session refresh failed. Please try again.', 'AUTH_REFRESH_FAILED');
      }
    }

    invalidateSession();
  }

  return response;
}

async function handleResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    // Try to parse standardized error
    try {
      const errorBody: ApiResponse<null> = await response.json();
      throw new ApiError(response.status, errorBody.message || response.statusText, errorBody.code);
    } catch (e) {
      if (e instanceof ApiError) throw e;
      throw new ApiError(response.status, response.statusText);
    }
  }

  // Parse result
  try {
    const body: ApiResponse<T> = await response.json();
    if (!body.success) {
      // Should theoretically be caught by !response.ok if status code matches, but handling logical errors
      throw new ApiError(200, body.message, body.code);
    }
    return body.data;
  } catch (e) {
    if (e instanceof ApiError) throw e;
    throw new Error("Failed to parse API response");
  }
}

export async function apiGet<T>(path: string): Promise<T> {
  const response = await authenticatedFetch(path);
  return handleResponse<T>(response);
}

export async function apiGetFull<T>(path: string): Promise<ApiResponse<T>> {
  const response = await authenticatedFetch(path);
  return handleFullResponse<ApiResponse<T>>(response);
}

export async function apiPostFull<T>(path: string, data: unknown): Promise<ApiResponse<T>> {
  const response = await authenticatedFetch(path, {
    method: 'POST',
    body: JSON.stringify(data),
  });
  return handleFullResponse<ApiResponse<T>>(response);
}

export async function apiPost<T>(path: string, data: unknown): Promise<T> {
  const response = await authenticatedFetch(path, {
    method: 'POST',
    body: JSON.stringify(data),
  });
  return handleResponse<T>(response);
}

export async function apiPut<T>(path: string, data: unknown): Promise<T> {
  const response = await authenticatedFetch(path, {
    method: 'PUT',
    body: JSON.stringify(data),
  });
  return handleResponse<T>(response);
}

export async function apiPatch<T>(path: string, data: unknown): Promise<T> {
  const response = await authenticatedFetch(path, {
    method: 'PATCH',
    body: JSON.stringify(data),
  });
  return handleResponse<T>(response);
}

export async function apiDelete(path: string): Promise<void> {
  const response = await authenticatedFetch(path, {
    method: 'DELETE',
  });
  return handleResponse<void>(response);
}

/**
 * Custom fetch mutator for Orval-generated API clients
 * Integrates with our existing auth and error handling
 *
 * Returns the FULL response body (not just data) since Orval-generated
 * types expect the complete ApiResponse structure with success, code, data, etc.
 */
export async function customFetch<T>(
  config: {
    url: string;
    method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'PATCH';
    headers?: Record<string, string>;
    params?: Record<string, unknown>;
    data?: unknown;
    signal?: AbortSignal;
  },
  options?: RequestInit
): Promise<T> {
  // Path: Orval uses relative paths like /api/v1/projects. If config.url is full URL (starts with API_BASE_URL), strip base; else use as-is.
  let path =
    API_BASE_URL && config.url.startsWith(API_BASE_URL)
      ? config.url.slice(API_BASE_URL.length) || '/'
      : config.url;

  // Add query parameters if present
  if (config.params) {
    const searchParams = new URLSearchParams();
    Object.entries(config.params).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        searchParams.append(key, String(value));
      }
    });
    const queryString = searchParams.toString();
    if (queryString) {
      path += `?${queryString}`;
    }
  }

  const requestOptions: RequestInit = {
    ...options,
    method: config.method,
    signal: config.signal,
  };

  // Add body for non-GET requests
  if (config.data && config.method !== 'GET') {
    requestOptions.body = JSON.stringify(config.data);
  }

  // Merge headers
  if (config.headers) {
    requestOptions.headers = {
      ...requestOptions.headers,
      ...config.headers,
    };
  }

  const response = await authenticatedFetch(path, requestOptions);

  // Return FULL response body for Orval-generated types
  // (they expect { success, code, message, data, ... } structure)
  return handleFullResponse<T>(response);
}

/**
 * Handle response and return FULL body (for Orval-generated clients)
 */
async function handleFullResponse<T>(response: Response): Promise<T> {
  if (!response.ok) {
    try {
      const errorBody: ApiResponse<null> = await response.json();
      throw new ApiError(response.status, errorBody.message || response.statusText, errorBody.code);
    } catch (e) {
      if (e instanceof ApiError) throw e;
      throw new ApiError(response.status, response.statusText);
    }
  }

  try {
    const body = await response.json();
    if (!body.success) {
      throw new ApiError(200, body.message, body.code);
    }
    // Return full response body, not just data
    return body as T;
  } catch (e) {
    if (e instanceof ApiError) throw e;
    throw new Error("Failed to parse API response");
  }
}
