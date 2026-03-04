import { useState, useEffect } from 'react';
import { apiGet, apiPost, API_PREFIX } from '../api/client';
import { Sprint } from '../shared/types';
import { logger } from '@/lib/logger';

export function useSprints(projectId?: string) {
    const [sprints, setSprints] = useState<Sprint[]>([]);
    const [loading, setLoading] = useState(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        if (projectId) {
            fetchSprints(projectId);
        } else {
            setSprints([]);
        }
    }, [projectId]);

    const fetchSprints = async (pId: string) => {
        setLoading(true);
        try {
            const data = await apiGet<Sprint[]>(`${API_PREFIX}/projects/${pId}/sprints`);
            setSprints(data);
            setError(null);
        } catch (err) {
            logger.error('Failed to fetch sprints:', err);
            setError('Failed to fetch sprints');
        } finally {
            setLoading(false);
        }
    };

    const generateSprints = async (pId: string, startDate: string, durationWeeks: number, count: number) => {
        try {
            await apiPost(`${API_PREFIX}/projects/${pId}/sprints/generate`, {
                start_date: startDate,
                duration_weeks: durationWeeks,
                count
            });
            await fetchSprints(pId);
        } catch (err) {
            logger.error('Failed to generate sprints:', err);
            throw err;
        }
    };

    return {
        sprints,
        loading,
        error,
        refreshSprints: fetchSprints,
        generateSprints
    };
}
