import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
    createOpenClawBootstrapToken,
    deleteOpenClawClient,
    disableOpenClawClient,
    enableOpenClawClient,
    listOpenClawClients,
    revokeOpenClawClient,
    type CreateOpenClawBootstrapTokenRequest,
    type OpenClawBootstrapPromptResponse,
    type OpenClawClientSummary,
} from '../api/openclawAdmin';

const OPENCLAW_CLIENTS_QUERY_KEY = ['openclaw-admin', 'clients'];

export interface UseOpenClawAccessResult {
    clients: OpenClawClientSummary[];
    loading: boolean;
    error: string | null;
    latestPrompt: OpenClawBootstrapPromptResponse | null;
    creatingPrompt: boolean;
    activeClientMutationId: string | null;
    refetchClients: () => Promise<unknown>;
    clearLatestPrompt: () => void;
    generateBootstrapPrompt: (
        payload: CreateOpenClawBootstrapTokenRequest
    ) => Promise<OpenClawBootstrapPromptResponse>;
    deleteClient: (clientId: string) => Promise<void>;
    disableClient: (clientId: string) => Promise<void>;
    enableClient: (clientId: string) => Promise<void>;
    revokeClient: (clientId: string) => Promise<void>;
}

export function useOpenClawAccess(enabled: boolean): UseOpenClawAccessResult {
    const queryClient = useQueryClient();
    const [latestPrompt, setLatestPrompt] =
        useState<OpenClawBootstrapPromptResponse | null>(null);
    const [activeClientMutationId, setActiveClientMutationId] = useState<string | null>(null);
    const [localPendingClients, setLocalPendingClients] = useState<OpenClawClientSummary[]>([]);

    const clientsQuery = useQuery<OpenClawClientSummary[], Error>({
        queryKey: OPENCLAW_CLIENTS_QUERY_KEY,
        queryFn: async () => (await listOpenClawClients()).clients,
        enabled,
        staleTime: 30 * 1000,
    });

    const createPromptMutation = useMutation({
        mutationFn: createOpenClawBootstrapToken,
        onSuccess: async (prompt, variables) => {
            setLatestPrompt(prompt);
            setLocalPendingClients((current) => {
                const pendingClientId = `pending:${prompt.bootstrap_token_id}`;
                if (current.some((client) => client.client_id === pendingClientId)) {
                    return current;
                }

                return [
                    {
                        client_id: pendingClientId,
                        display_name:
                            variables.suggested_display_name?.trim() || variables.label.trim(),
                        status: 'waiting_connection',
                        kind: 'pending',
                        enrolled_at: new Date().toISOString(),
                        expires_at: prompt.expires_at,
                        last_seen_at: null,
                        last_seen_ip: null,
                        last_seen_user_agent: null,
                        key_fingerprints: [],
                    },
                    ...current,
                ];
            });
            await queryClient.invalidateQueries({ queryKey: OPENCLAW_CLIENTS_QUERY_KEY });
        },
    });

    const refreshClients = async () => {
        await queryClient.invalidateQueries({ queryKey: OPENCLAW_CLIENTS_QUERY_KEY });
        return clientsQuery.refetch();
    };

    const mutateClient = async (
        clientId: string,
        mutation: (targetClientId: string) => Promise<unknown>
    ) => {
        setActiveClientMutationId(clientId);
        try {
            await mutation(clientId);
            setLocalPendingClients((current) =>
                current.filter((client) => client.client_id !== clientId)
            );
            await refreshClients();
        } finally {
            setActiveClientMutationId(null);
        }
    };

    const mergedClients = useMemo(() => {
        const serverClients = clientsQuery.data ?? [];
        const merged = [...serverClients];

        for (const pendingClient of localPendingClients) {
            if (!merged.some((client) => client.client_id === pendingClient.client_id)) {
                merged.push(pendingClient);
            }
        }

        return merged;
    }, [clientsQuery.data, localPendingClients]);

    return {
        clients: mergedClients,
        loading: clientsQuery.isLoading,
        error: clientsQuery.error ? clientsQuery.error.message : null,
        latestPrompt,
        creatingPrompt: createPromptMutation.isPending,
        activeClientMutationId,
        refetchClients: refreshClients,
        clearLatestPrompt: () => setLatestPrompt(null),
        generateBootstrapPrompt: async (payload) =>
            createPromptMutation.mutateAsync(payload),
        deleteClient: async (clientId: string) =>
            mutateClient(clientId, deleteOpenClawClient),
        disableClient: async (clientId: string) =>
            mutateClient(clientId, disableOpenClawClient),
        enableClient: async (clientId: string) =>
            mutateClient(clientId, enableOpenClawClient),
        revokeClient: async (clientId: string) =>
            mutateClient(clientId, revokeOpenClawClient),
    };
}
