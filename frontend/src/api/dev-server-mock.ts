import { DevServerStatus } from '@/components/preview/DevServerControls';
import { logger } from '@/lib/logger';

/**
 * Mock API for dev server operations
 * This simulates backend responses for Phase 5.5 (frontend-only)
 * Replace with real API calls when backend is ready
 */

export interface DevServerApiResponse {
  status: DevServerStatus;
  url?: string;
  message?: string;
}

/**
 * Mock: Start dev server
 * Simulates 1.5s delay with 80% success rate
 */
export async function mockStartDevServer(taskId: string): Promise<DevServerApiResponse> {
  logger.log('[MOCK] Starting dev server for task:', taskId);

  // Simulate API delay
  await new Promise((resolve) => setTimeout(resolve, 1500));

  // Random success/failure (80% success)
  const shouldSucceed = Math.random() > 0.2;

  if (shouldSucceed) {
    const mockPort = 3000 + Math.floor(Math.random() * 1000);
    const mockUrl = `http://localhost:${mockPort}`;

    logger.log('[MOCK] Dev server started:', mockUrl);
    return {
      status: 'running',
      url: mockUrl,
    };
  } else {
    logger.error('[MOCK] Dev server failed to start');
    return {
      status: 'error',
      message: 'Port 3000 is already in use',
    };
  }
}

/**
 * Mock: Stop dev server
 * Simulates 800ms delay
 */
export async function mockStopDevServer(taskId: string): Promise<DevServerApiResponse> {
  logger.log('[MOCK] Stopping dev server for task:', taskId);

  // Simulate API delay
  await new Promise((resolve) => setTimeout(resolve, 800));

  logger.log('[MOCK] Dev server stopped');
  return {
    status: 'idle',
  };
}

/**
 * Mock: Get dev server status
 */
export async function mockGetDevServerStatus(taskId: string): Promise<DevServerApiResponse> {
  logger.log('[MOCK] Getting dev server status for task:', taskId);

  // Check localStorage for cached URL
  const cachedUrl = localStorage.getItem(`devServer:${taskId}:url`);

  if (cachedUrl) {
    return {
      status: 'running',
      url: cachedUrl,
    };
  }

  return {
    status: 'idle',
  };
}

/**
 * Future: Real API implementations
 * Uncomment and implement when backend is ready
 */

// import axios from 'axios';
//
// export async function startDevServer(taskId: string): Promise<DevServerApiResponse> {
//   const response = await axios.post(`/api/v1/tasks/${taskId}/preview/start`);
//   return response.data;
// }
//
// export async function stopDevServer(taskId: string): Promise<DevServerApiResponse> {
//   const response = await axios.post(`/api/v1/tasks/${taskId}/preview/stop`);
//   return response.data;
// }
//
// export async function getDevServerStatus(taskId: string): Promise<DevServerApiResponse> {
//   const response = await axios.get(`/api/v1/tasks/${taskId}/preview/status`);
//   return response.data;
// }
