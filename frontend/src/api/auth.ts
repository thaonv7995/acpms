import { apiPost, setAccessToken, setRefreshToken, clearTokens } from './client';
import type { UserDto } from './generated/models';

const CURRENT_USER_KEY = 'acpms_current_user';

export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: UserDto;
}

export interface RegisterRequest {
  email: string;
  name: string;
  password: string;
}

export interface LoginRequest {
  email: string;
  password: string;
}

export async function register(data: RegisterRequest): Promise<AuthResponse> {
  const response = await apiPost<AuthResponse>('/api/v1/auth/register', data);
  setAccessToken(response.access_token);
  setRefreshToken(response.refresh_token);
  setCurrentUser(response.user);
  return response;
}

export async function login(data: LoginRequest): Promise<AuthResponse> {
  const response = await apiPost<AuthResponse>('/api/v1/auth/login', data);
  setAccessToken(response.access_token);
  setRefreshToken(response.refresh_token);
  setCurrentUser(response.user);
  return response;
}

export function logout(): void {
  clearTokens();
  clearCurrentUser();
  window.location.href = '/login';
}

export function isAuthenticated(): boolean {
  return localStorage.getItem('acpms_token') !== null;
}

export function setCurrentUser(user: UserDto): void {
  localStorage.setItem(CURRENT_USER_KEY, JSON.stringify(user));
}

export function getCurrentUser(): UserDto | null {
  const userJson = localStorage.getItem(CURRENT_USER_KEY);
  if (!userJson) return null;
  try {
    return JSON.parse(userJson) as UserDto;
  } catch {
    return null;
  }
}

export function isSystemAdmin(user: UserDto | null = getCurrentUser()): boolean {
  return user?.global_roles?.includes('admin') ?? false;
}

export function clearCurrentUser(): void {
  localStorage.removeItem(CURRENT_USER_KEY);
}
