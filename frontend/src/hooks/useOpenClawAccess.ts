import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
    createOpenClawBootstrapToken,
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
    disableClient: (clientId: string) => Promise<void>;
    enableClient: (clientId: string) => Promise<void>;
    revokeClient: (clientId: string) => Promise<void>;
}

export function useOpenClawAccess(enabled: boolean): UseOpenClawAccessResult {
    const queryClient = useQueryClient();
    const [latestPrompt, setLatestPrompt] =
        useState<OpenClawBootstrapPromptResponse | null>(null);
    const [activeClientMutationId, setActiveClientMutationId] = useState<string | null>(null);

    const clientsQuery = useQuery<OpenClawClientSummary[], Error>({
        queryKey: OPENCLAW_CLIENTS_QUERY_KEY,
        queryFn: async () => (await listOpenClawClients()).clients,
        enabled,
        staleTime: 30 * 1000,
    });

    const createPromptMutation = useMutation({
        mutationFn: createOpenClawBootstrapToken,
        onSuccess: (prompt) => {
            setLatestPrompt(prompt);
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
            await refreshClients();
        } finally {
            setActiveClientMutationId(null);
        }
    };

    return {
        clients: clientsQuery.data ?? [],
        loading: clientsQuery.isLoading,
        error: clientsQuery.error ? clientsQuery.error.message : null,
        latestPrompt,
        creatingPrompt: createPromptMutation.isPending,
        activeClientMutationId,
        refetchClients: refreshClients,
        clearLatestPrompt: () => setLatestPrompt(null),
        generateBootstrapPrompt: async (payload) =>
            createPromptMutation.mutateAsync(payload),
        disableClient: async (clientId: string) =>
            mutateClient(clientId, disableOpenClawClient),
        enableClient: async (clientId: string) =>
            mutateClient(clientId, enableOpenClawClient),
        revokeClient: async (clientId: string) =>
            mutateClient(clientId, revokeOpenClawClient),
    };
}
