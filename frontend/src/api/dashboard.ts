import { apiGet } from './client';
import type { DashboardData } from '../types/dashboard';

export async function getDashboardData(): Promise<DashboardData> {
    return apiGet<DashboardData>('/dashboard');
}

