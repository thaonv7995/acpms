import { useCallback, useEffect, useMemo, useState } from 'react';
import {
    addEdge,
    Background,
    Connection,
    Controls,
    EdgeChange,
    Edge,
    Handle,
    MarkerType,
    MiniMap,
    NodeChange,
    Node,
    Position,
    ReactFlow,
    useEdgesState,
    useNodesState,
} from '@xyflow/react';
import * as dagre from '@dagrejs/dagre';
import '@xyflow/react/dist/style.css';
import { apiGet, apiPut } from '@/api/client';
import { createTask } from '@/api/tasks';
import { logger } from '@/lib/logger';

type ArchitectureStatus = 'healthy' | 'warning' | 'error';

const NODE_TYPES = [
    'api',
    'database',
    'cache',
    'queue',
    'storage',
    'frontend',
    'worker',
    'auth',
    'gateway',
    'service',
    'client',
    'mobile',
] as const;

type ArchitectureNodeType = (typeof NODE_TYPES)[number];

interface ArchitectureNodeConfig {
    id: string;
    label: string;
    type: ArchitectureNodeType;
    status?: ArchitectureStatus;
}

interface ArchitectureEdgeConfig {
    source: string;
    target: string;
    label?: string;
}

interface ArchitectureConfig {
    nodes: ArchitectureNodeConfig[];
    edges: ArchitectureEdgeConfig[];
}

interface ArchitectureDiffSummary {
    addedNodes: string[];
    removedNodes: string[];
    changedNodes: string[];
    addedEdges: string[];
    removedEdges: string[];
}

interface PendingArchitectureSync {
    oldConfig: ArchitectureConfig;
    newConfig: ArchitectureConfig;
    diff: ArchitectureDiffSummary;
}

interface AddComponentDraft {
    label: string;
    type: ArchitectureNodeType;
}

interface ArchitectureTabProps {
    projectId: string;
}

const typeIcons: Record<ArchitectureNodeType, string> = {
    api: 'api',
    database: 'database',
    cache: 'memory',
    queue: 'queue',
    storage: 'cloud_upload',
    frontend: 'web',
    worker: 'engineering',
    auth: 'lock',
    gateway: 'router',
    service: 'dns',
    client: 'laptop_mac',
    mobile: 'smartphone',
};

const typeColors: Record<ArchitectureNodeType, { border: string; bg: string; text: string }> = {
    api: { border: 'border-blue-500', bg: 'bg-blue-50 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400' },
    gateway: { border: 'border-blue-500', bg: 'bg-blue-50 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400' },
    database: { border: 'border-amber-500', bg: 'bg-amber-50 dark:bg-amber-500/20', text: 'text-amber-600 dark:text-amber-400' },
    cache: { border: 'border-red-500', bg: 'bg-red-50 dark:bg-red-500/20', text: 'text-red-600 dark:text-red-400' },
    queue: { border: 'border-purple-500', bg: 'bg-purple-50 dark:bg-purple-500/20', text: 'text-purple-600 dark:text-purple-400' },
    storage: { border: 'border-green-500', bg: 'bg-green-50 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400' },
    frontend: { border: 'border-cyan-500', bg: 'bg-cyan-50 dark:bg-cyan-500/20', text: 'text-cyan-600 dark:text-cyan-400' },
    worker: { border: 'border-orange-500', bg: 'bg-orange-50 dark:bg-orange-500/20', text: 'text-orange-600 dark:text-orange-400' },
    auth: { border: 'border-pink-500', bg: 'bg-pink-50 dark:bg-pink-500/20', text: 'text-pink-600 dark:text-pink-400' },
    service: { border: 'border-indigo-500', bg: 'bg-indigo-50 dark:bg-indigo-500/20', text: 'text-indigo-600 dark:text-indigo-400' },
    client: { border: 'border-border', bg: 'bg-muted', text: 'text-muted-foreground' },
    mobile: { border: 'border-border', bg: 'bg-muted', text: 'text-muted-foreground' },
};

function isRecord(value: unknown): value is Record<string, unknown> {
    return typeof value === 'object' && value !== null;
}

function normalizeNodeType(value: unknown): ArchitectureNodeType {
    if (typeof value !== 'string') return 'service';
    const normalized = value.trim().toLowerCase();
    return (NODE_TYPES as readonly string[]).includes(normalized) ? (normalized as ArchitectureNodeType) : 'service';
}

function normalizeStatus(value: unknown): ArchitectureStatus | undefined {
    if (value === 'healthy' || value === 'warning' || value === 'error') return value;
    return undefined;
}

function normalizeArchitectureConfig(raw: unknown): ArchitectureConfig {
    if (!isRecord(raw)) {
        return { nodes: [], edges: [] };
    }

    const rawNodes = Array.isArray(raw.nodes) ? raw.nodes : [];
    const normalizedNodes: ArchitectureNodeConfig[] = [];
    const seenNodeIds = new Set<string>();

    for (let index = 0; index < rawNodes.length; index += 1) {
        const candidate = rawNodes[index];
        if (!isRecord(candidate)) continue;

        const rawId = typeof candidate.id === 'string' ? candidate.id.trim() : '';
        const nodeId = rawId || `node-${index + 1}`;
        if (seenNodeIds.has(nodeId)) continue;

        const rawLabel = typeof candidate.label === 'string' ? candidate.label.trim() : '';
        const nodeType = normalizeNodeType(candidate.type);
        const status = normalizeStatus(candidate.status);

        normalizedNodes.push({
            id: nodeId,
            label: rawLabel || nodeId,
            type: nodeType,
            status,
        });
        seenNodeIds.add(nodeId);
    }

    const nodeIdSet = new Set(normalizedNodes.map((node) => node.id));
    const rawEdges = Array.isArray(raw.edges) ? raw.edges : [];
    const normalizedEdges: ArchitectureEdgeConfig[] = [];
    const seenEdges = new Set<string>();

    for (const candidate of rawEdges) {
        if (!isRecord(candidate)) continue;
        const source = typeof candidate.source === 'string' ? candidate.source.trim() : '';
        const target = typeof candidate.target === 'string' ? candidate.target.trim() : '';
        if (!source || !target) continue;
        if (!nodeIdSet.has(source) || !nodeIdSet.has(target)) continue;

        const label = typeof candidate.label === 'string' ? candidate.label.trim() : '';
        const key = `${source}->${target}|${label}`;
        if (seenEdges.has(key)) continue;

        normalizedEdges.push({
            source,
            target,
            ...(label ? { label } : {}),
        });
        seenEdges.add(key);
    }

    return { nodes: normalizedNodes, edges: normalizedEdges };
}

function toFlowElements(config: ArchitectureConfig): { nodes: Node[]; edges: Edge[] } {
    const nodes: Node[] = config.nodes.map((node) => ({
        id: node.id,
        type: 'architecture',
        position: { x: 0, y: 0 },
        data: { label: node.label, nodeType: node.type, status: node.status },
    }));

    const edges: Edge[] = config.edges.map((edge, index) => ({
        id: `e-${edge.source}-${edge.target}-${index}`,
        source: edge.source,
        target: edge.target,
        label: edge.label,
        markerEnd: { type: MarkerType.ArrowClosed, color: '#94a3b8' },
        style: { stroke: '#94a3b8', strokeWidth: 2 },
        animated: true,
    }));

    return getLayoutedElements(nodes, edges);
}

function serializeArchitectureConfig(nodes: Node[], edges: Edge[]): ArchitectureConfig {
    const serializedNodes: ArchitectureNodeConfig[] = nodes.map((node) => {
        const data = isRecord(node.data) ? node.data : {};
        const nodeType = normalizeNodeType(data.nodeType);
        const status = normalizeStatus(data.status);
        const label = typeof data.label === 'string' && data.label.trim() ? data.label.trim() : node.id;

        return {
            id: node.id,
            label,
            type: nodeType,
            ...(status ? { status } : {}),
        };
    });

    const nodeIdSet = new Set(serializedNodes.map((node) => node.id));
    const dedupeEdges = new Set<string>();
    const serializedEdges: ArchitectureEdgeConfig[] = [];

    for (const edge of edges) {
        if (!nodeIdSet.has(edge.source) || !nodeIdSet.has(edge.target)) continue;
        const label = typeof edge.label === 'string' ? edge.label.trim() : '';
        const key = `${edge.source}->${edge.target}|${label}`;
        if (dedupeEdges.has(key)) continue;

        serializedEdges.push({
            source: edge.source,
            target: edge.target,
            ...(label ? { label } : {}),
        });
        dedupeEdges.add(key);
    }

    return { nodes: serializedNodes, edges: serializedEdges };
}

function canonicalizeConfig(config: ArchitectureConfig): ArchitectureConfig {
    const nodes = [...config.nodes]
        .map((node) => ({
            id: node.id,
            label: node.label,
            type: normalizeNodeType(node.type),
            ...(node.status ? { status: node.status } : {}),
        }))
        .sort((a, b) => a.id.localeCompare(b.id));

    const edges = [...config.edges]
        .map((edge) => ({
            source: edge.source,
            target: edge.target,
            ...(edge.label ? { label: edge.label } : {}),
        }))
        .sort((a, b) => {
            const left = `${a.source}->${a.target}|${a.label || ''}`;
            const right = `${b.source}->${b.target}|${b.label || ''}`;
            return left.localeCompare(right);
        });

    return { nodes, edges };
}

function configsEqual(left: ArchitectureConfig, right: ArchitectureConfig): boolean {
    return JSON.stringify(canonicalizeConfig(left)) === JSON.stringify(canonicalizeConfig(right));
}

function diffArchitecture(oldConfig: ArchitectureConfig, newConfig: ArchitectureConfig): ArchitectureDiffSummary {
    const oldNodes = new Map(oldConfig.nodes.map((node) => [node.id, node]));
    const newNodes = new Map(newConfig.nodes.map((node) => [node.id, node]));

    const addedNodes = newConfig.nodes
        .filter((node) => !oldNodes.has(node.id))
        .map((node) => `${node.id} (${node.type})`);

    const removedNodes = oldConfig.nodes
        .filter((node) => !newNodes.has(node.id))
        .map((node) => `${node.id} (${node.type})`);

    const changedNodes = newConfig.nodes
        .filter((node) => {
            const oldNode = oldNodes.get(node.id);
            return (
                !!oldNode &&
                (oldNode.label !== node.label || oldNode.type !== node.type || oldNode.status !== node.status)
            );
        })
        .map((node) => `${node.id} (${node.type})`);

    const oldEdges = new Set(oldConfig.edges.map((edge) => `${edge.source}->${edge.target}|${edge.label || ''}`));
    const newEdges = new Set(newConfig.edges.map((edge) => `${edge.source}->${edge.target}|${edge.label || ''}`));

    const addedEdges = [...newEdges]
        .filter((edge) => !oldEdges.has(edge))
        .map((edge) => edge.replace('|', edge.endsWith('|') ? '' : ' | '));
    const removedEdges = [...oldEdges]
        .filter((edge) => !newEdges.has(edge))
        .map((edge) => edge.replace('|', edge.endsWith('|') ? '' : ' | '));

    return { addedNodes, removedNodes, changedNodes, addedEdges, removedEdges };
}

function buildArchitectureSyncTaskDescription(
    oldConfig: ArchitectureConfig,
    newConfig: ArchitectureConfig,
    diff: ArchitectureDiffSummary
): string {
    const oldNodeIds = new Set(oldConfig.nodes.map((node) => node.id));
    const addedBackendNodes = newConfig.nodes
        .filter((node) => !oldNodeIds.has(node.id))
        .filter((node) =>
            ['api', 'service', 'auth', 'gateway', 'worker', 'database', 'cache', 'queue', 'storage'].includes(
                normalizeNodeType(node.type)
            )
        )
        .map((node) => `${node.label} [${node.id}] (${normalizeNodeType(node.type)})`);

    const implementationRules = [
        'Instruction: Apply code changes in repository to align with target architecture.',
        'Do not call architecture APIs from this task.',
        'Prioritize architecture delta implementation over unrelated tweaks in existing files.',
        'Keep unrelated extension/frontend files unchanged unless required by architecture wiring.',
    ];

    if (addedBackendNodes.length > 0) {
        implementationRules.push(
            `Required backend/infrastructure components to implement: ${addedBackendNodes.join(', ')}`
        );
        implementationRules.push(
            'You MUST create/update concrete backend/service code paths (modules, routes, config, tests as needed), not only extension UI files.'
        );
    }

    return [
        'Architecture Change Task',
        '',
        'Summary of detected changes:',
        diff.addedNodes.length ? `- Added nodes: ${diff.addedNodes.join(', ')}` : '- Added nodes: none',
        diff.removedNodes.length ? `- Removed nodes: ${diff.removedNodes.join(', ')}` : '- Removed nodes: none',
        diff.changedNodes.length ? `- Changed nodes: ${diff.changedNodes.join(', ')}` : '- Changed nodes: none',
        diff.addedEdges.length ? `- Added edges: ${diff.addedEdges.join(', ')}` : '- Added edges: none',
        diff.removedEdges.length ? `- Removed edges: ${diff.removedEdges.join(', ')}` : '- Removed edges: none',
        '',
        'Current architecture (JSON):',
        JSON.stringify(oldConfig, null, 2),
        '',
        'Target architecture (JSON):',
        JSON.stringify(newConfig, null, 2),
        '',
        ...implementationRules,
        'Final report requirement: map each added/changed architecture component to changed/created files.',
    ].join('\n');
}

function toSafeNodeIdBase(label: string, fallbackType: ArchitectureNodeType): string {
    const normalized = label
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, '-')
        .replace(/^-+|-+$/g, '');
    return normalized || fallbackType;
}

function generateUniqueNodeId(existingNodes: Node[], label: string, type: ArchitectureNodeType): string {
    const existingIds = new Set(existingNodes.map((node) => node.id));
    const base = toSafeNodeIdBase(label, type);
    if (!existingIds.has(base)) return base;

    let suffix = 2;
    while (existingIds.has(`${base}-${suffix}`)) {
        suffix += 1;
    }
    return `${base}-${suffix}`;
}

function getNewNodePosition(existingNodes: Node[]): { x: number; y: number } {
    if (existingNodes.length === 0) {
        return { x: 180, y: 140 };
    }

    const maxY = Math.max(...existingNodes.map((node) => node.position.y));
    const avgX = existingNodes.reduce((sum, node) => sum + node.position.x, 0) / existingNodes.length;

    return {
        x: Math.round(avgX + 40),
        y: Math.round(maxY + 140),
    };
}

function ArchitectureNodeComponent({ data }: { data: { label: string; nodeType: ArchitectureNodeType; status?: ArchitectureStatus } }) {
    const nodeType = normalizeNodeType(data.nodeType);
    const colors = typeColors[nodeType];
    const icon = typeIcons[nodeType];
    const statusColor = data.status === 'healthy' ? 'bg-green-500' : data.status === 'warning' ? 'bg-yellow-500' : data.status === 'error' ? 'bg-red-500' : null;

    return (
        <div className={`px-4 py-3 rounded-lg border-2 ${colors.border} ${colors.bg} shadow-sm min-w-[120px] relative`}>
            <Handle type="target" position={Position.Top} className="!bg-slate-400" />
            <div className="flex items-center gap-2">
                <span className={`material-symbols-outlined text-lg ${colors.text}`}>{icon}</span>
                <span className="text-sm font-medium text-card-foreground">{data.label}</span>
            </div>
            {statusColor && (
                <div className="absolute -right-1 -top-1 flex h-3 w-3">
                    <span className={`animate-ping absolute inline-flex h-full w-full rounded-full ${statusColor} opacity-75`}></span>
                    <span className={`relative inline-flex rounded-full h-3 w-3 ${statusColor}`}></span>
                </div>
            )}
            <Handle type="source" position={Position.Bottom} className="!bg-slate-400" />
        </div>
    );
}

const nodeTypes = {
    architecture: ArchitectureNodeComponent,
};

function getLayoutedElements(nodes: Node[], edges: Edge[], direction = 'TB') {
    if (!nodes.length) return { nodes, edges };

    try {
        const dagreInstance = (dagre as any).default || dagre;
        const dagreGraph = new dagreInstance.graphlib.Graph();
        dagreGraph.setDefaultEdgeLabel(() => ({}));
        dagreGraph.setGraph({ rankdir: direction, nodesep: 80, ranksep: 100 });

        nodes.forEach((node) => {
            dagreGraph.setNode(node.id, { width: 150, height: 60 });
        });

        edges.forEach((edge) => {
            dagreGraph.setEdge(edge.source, edge.target);
        });

        dagreInstance.layout(dagreGraph);

        const layoutedNodes = nodes.map((node) => {
            const nodeWithPosition = dagreGraph.node(node.id);
            if (!nodeWithPosition) return node;

            return {
                ...node,
                position: {
                    x: nodeWithPosition.x - 75,
                    y: nodeWithPosition.y - 30,
                },
            };
        });

        return { nodes: layoutedNodes, edges };
    } catch (err) {
        logger.error('Failed to layout with dagre:', err);
        return { nodes, edges };
    }
}

function buildStarterArchitecture(): ArchitectureConfig {
    return {
        nodes: [
            { id: 'browser', label: 'Browser Client', type: 'client', status: 'healthy' },
            { id: 'frontend', label: 'Web Frontend', type: 'frontend', status: 'healthy' },
            { id: 'api', label: 'Application API', type: 'api', status: 'healthy' },
            { id: 'database', label: 'Primary Database', type: 'database', status: 'healthy' },
        ],
        edges: [
            { source: 'browser', target: 'frontend', label: 'HTTPS' },
            { source: 'frontend', target: 'api', label: 'REST/GraphQL' },
            { source: 'api', target: 'database', label: 'Read/Write' },
        ],
    };
}

export function ArchitectureTab({ projectId }: ArchitectureTabProps) {
    const [persistedConfig, setPersistedConfig] = useState<ArchitectureConfig>({ nodes: [], edges: [] });
    const [loading, setLoading] = useState(true);
    const [saving, setSaving] = useState(false);
    const [isManualEditMode, setIsManualEditMode] = useState(false);
    const [creatingSyncTask, setCreatingSyncTask] = useState(false);
    const [showAddComponentModal, setShowAddComponentModal] = useState(false);
    const [pendingSync, setPendingSync] = useState<PendingArchitectureSync | null>(null);
    const [addComponentDraft, setAddComponentDraft] = useState<AddComponentDraft>({
        label: '',
        type: 'service',
    });
    const [notice, setNotice] = useState<{ type: 'success' | 'error' | 'info'; message: string } | null>(null);

    useEffect(() => {
        let cancelled = false;
        const fetchConfig = async () => {
            try {
                const data = await apiGet<unknown>(`/api/v1/projects/${projectId}/architecture`);
                if (cancelled) return;
                setPersistedConfig(normalizeArchitectureConfig(data));
            } catch (err) {
                logger.error('Error fetching architecture:', err);
                if (!cancelled) {
                    setPersistedConfig({ nodes: [], edges: [] });
                    setNotice({ type: 'error', message: 'Failed to load architecture. Showing empty diagram.' });
                }
            } finally {
                if (!cancelled) setLoading(false);
            }
        };

        fetchConfig();
        return () => {
            cancelled = true;
        };
    }, [projectId]);

    const { initialNodes, initialEdges } = useMemo(() => {
        const layouted = toFlowElements(persistedConfig);
        return { initialNodes: layouted.nodes, initialEdges: layouted.edges };
    }, [persistedConfig]);

    const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
    const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

    const handleNodesChange = useCallback(
        (changes: NodeChange[]) => {
            if (isManualEditMode) {
                onNodesChange(changes);
                return;
            }

            // View mode: allow visual reposition only, block structural edits.
            const viewOnlyChanges = changes.filter(
                (change) => change.type === 'position' || change.type === 'dimensions'
            );
            if (viewOnlyChanges.length > 0) {
                onNodesChange(viewOnlyChanges);
            }
        },
        [isManualEditMode, onNodesChange]
    );

    const handleEdgesChange = useCallback(
        (changes: EdgeChange[]) => {
            if (!isManualEditMode) return;
            onEdgesChange(changes);
        },
        [isManualEditMode, onEdgesChange]
    );

    useEffect(() => {
        setNodes(initialNodes);
        setEdges(initialEdges);
    }, [initialNodes, initialEdges, setNodes, setEdges]);

    const draftConfig = useMemo(() => serializeArchitectureConfig(nodes, edges), [nodes, edges]);
    const hasUnsavedChanges = useMemo(
        () => !configsEqual(persistedConfig, draftConfig),
        [persistedConfig, draftConfig]
    );

    const onLayout = useCallback(() => {
        const layouted = getLayoutedElements(nodes, edges);
        setNodes([...layouted.nodes]);
        setEdges([...layouted.edges]);
        if (!isManualEditMode) {
            setNotice({ type: 'info', message: 'Layout updated. Enable Manual Edit and Save to persist.' });
        }
    }, [isManualEditMode, nodes, edges, setNodes, setEdges]);

    const onConnect = useCallback(
        (connection: Connection) => {
            if (!isManualEditMode) return;
            setEdges((existingEdges) =>
                addEdge(
                    {
                        ...connection,
                        markerEnd: { type: MarkerType.ArrowClosed, color: '#94a3b8' },
                        style: { stroke: '#94a3b8', strokeWidth: 2 },
                        animated: true,
                    },
                    existingEdges
                )
            );
        },
        [isManualEditMode, setEdges]
    );

    const handleSave = useCallback(async () => {
        if (!hasUnsavedChanges) {
            setNotice({ type: 'info', message: 'No architecture changes to save.' });
            return;
        }

        setSaving(true);
        setNotice(null);
        try {
            const previousConfig = canonicalizeConfig(persistedConfig);
            const updated = await apiPut<unknown>(`/api/v1/projects/${projectId}/architecture`, {
                config: draftConfig,
            });
            const savedConfig = normalizeArchitectureConfig(updated);
            setPersistedConfig(savedConfig);
            setNotice({ type: 'success', message: 'Architecture saved successfully.' });

            if (!configsEqual(previousConfig, savedConfig)) {
                setPendingSync({
                    oldConfig: previousConfig,
                    newConfig: savedConfig,
                    diff: diffArchitecture(previousConfig, savedConfig),
                });
            }
        } catch (err) {
            const message = err instanceof Error ? err.message : 'Failed to save architecture';
            setNotice({ type: 'error', message });
        } finally {
            setSaving(false);
        }
    }, [draftConfig, hasUnsavedChanges, persistedConfig, projectId]);

    const handleDiscard = useCallback(() => {
        setNodes(initialNodes);
        setEdges(initialEdges);
        setNotice({ type: 'info', message: 'Reverted unsaved architecture changes.' });
    }, [initialEdges, initialNodes, setEdges, setNodes]);

    const handleCreateStarter = useCallback(() => {
        const starter = toFlowElements(buildStarterArchitecture());
        setNodes(starter.nodes);
        setEdges(starter.edges);
        setIsManualEditMode(true);
        setNotice({ type: 'info', message: 'Starter architecture added. Click Save to persist.' });
    }, [setEdges, setNodes]);

    const handleCreateSyncTask = useCallback(async () => {
        if (!pendingSync) return;

        setCreatingSyncTask(true);
        try {
            const description = buildArchitectureSyncTaskDescription(
                pendingSync.oldConfig,
                pendingSync.newConfig,
                pendingSync.diff
            );

            const createdTask = await createTask({
                project_id: projectId,
                title: 'Apply architecture changes',
                description,
                task_type: 'feature',
                metadata: {
                    source: 'architecture_change',
                    old_architecture: pendingSync.oldConfig,
                    new_architecture: pendingSync.newConfig,
                    architecture_diff: pendingSync.diff,
                },
            });

            setNotice({
                type: 'success',
                message: `Architecture sync task created: ${createdTask.title}`,
            });
            setPendingSync(null);
        } catch (err) {
            const message = err instanceof Error ? err.message : 'Failed to create architecture sync task';
            setNotice({ type: 'error', message });
        } finally {
            setCreatingSyncTask(false);
        }
    }, [pendingSync, projectId]);

    const openAddComponentModal = useCallback(() => {
        if (!isManualEditMode) {
            setNotice({ type: 'info', message: 'Enable Manual Edit first to add component.' });
            return;
        }
        setAddComponentDraft({ label: '', type: 'service' });
        setShowAddComponentModal(true);
    }, [isManualEditMode]);

    const handleAddComponent = useCallback(() => {
        if (!isManualEditMode) return;

        const label = addComponentDraft.label.trim();
        if (!label) {
            setNotice({ type: 'error', message: 'Component label is required.' });
            return;
        }

        const nodeId = generateUniqueNodeId(nodes, label, addComponentDraft.type);
        const position = getNewNodePosition(nodes);

        const newNode: Node = {
            id: nodeId,
            type: 'architecture',
            position,
            data: {
                label,
                nodeType: addComponentDraft.type,
                status: 'healthy',
            },
        };

        setNodes((existingNodes) => [...existingNodes, newNode]);
        setShowAddComponentModal(false);
        setNotice({ type: 'success', message: `Added component: ${label}` });
    }, [addComponentDraft.label, addComponentDraft.type, isManualEditMode, nodes, setNodes]);

    if (loading) {
        return (
            <div className="h-[600px] flex items-center justify-center bg-card rounded-xl border border-border">
                <div className="flex flex-col items-center gap-3">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
                    <span className="text-sm text-muted-foreground">Loading architecture...</span>
                </div>
            </div>
        );
    }

    const isEmpty = draftConfig.nodes.length === 0;

    if (isEmpty) {
        return (
            <div className="bg-card border border-border rounded-xl overflow-hidden">
                <div className="flex justify-between items-center px-4 py-3 border-b border-border">
                    <h3 className="font-bold text-card-foreground flex items-center gap-2">
                        <span className="material-symbols-outlined text-primary">hub</span>
                        System Architecture
                    </h3>
                    <button
                        onClick={handleCreateStarter}
                        className="px-3 py-1.5 text-sm bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg flex items-center gap-1 transition-colors"
                    >
                        <span className="material-symbols-outlined text-[18px]">add</span>
                        Start Template
                    </button>
                </div>
                <div className="h-[500px] flex flex-col items-center justify-center bg-muted/30">
                    <span className="material-symbols-outlined text-6xl text-muted-foreground/50 mb-4">hub</span>
                    <h4 className="text-lg font-medium text-card-foreground mb-2">No architecture configured</h4>
                    <p className="text-sm text-muted-foreground mb-6 text-center max-w-md">
                        Start from a template, then edit and save your architecture diagram.
                    </p>
                    <button
                        onClick={handleCreateStarter}
                        className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg flex items-center gap-2 transition-colors"
                    >
                        <span className="material-symbols-outlined text-[18px]">auto_awesome</span>
                        Create Starter Diagram
                    </button>
                </div>
            </div>
        );
    }

    return (
        <div className="space-y-4">
            <div className="bg-card border border-border rounded-xl overflow-hidden">
                <div className="flex justify-between items-center px-4 py-3 border-b border-border">
                    <div className="flex items-center gap-3">
                        <h3 className="font-bold text-card-foreground flex items-center gap-2">
                            <span className="material-symbols-outlined text-primary">hub</span>
                            System Architecture
                        </h3>
                        <span className={`px-2 py-0.5 rounded text-xs border ${isManualEditMode ? 'bg-primary/20 text-primary border-primary/40' : 'bg-muted text-muted-foreground border-border'}`}>
                            {isManualEditMode ? 'Manual Edit' : 'View'}
                        </span>
                        {hasUnsavedChanges && (
                            <span className="px-2 py-0.5 rounded text-xs bg-amber-500/20 text-amber-400 border border-amber-500/40">
                                Unsaved changes
                            </span>
                        )}
                    </div>
                    <div className="flex gap-2">
                        <button
                            onClick={() => setIsManualEditMode((prev) => !prev)}
                            title={isManualEditMode ? 'Done Editing' : 'Manual Edit'}
                            className={`h-9 w-9 rounded-lg flex items-center justify-center transition-colors ${
                                isManualEditMode
                                    ? 'bg-primary/15 text-primary border border-primary/40'
                                    : 'bg-muted hover:bg-muted/80 text-card-foreground border border-border'
                            }`}
                        >
                            <span className="material-symbols-outlined text-[18px]">{isManualEditMode ? 'edit_off' : 'edit'}</span>
                        </button>
                        <button
                            onClick={openAddComponentModal}
                            disabled={!isManualEditMode}
                            title="Add Component"
                            className="h-9 w-9 bg-muted hover:bg-muted/80 disabled:opacity-50 disabled:cursor-not-allowed text-card-foreground rounded-lg flex items-center justify-center transition-colors"
                        >
                            <span className="material-symbols-outlined text-[18px]">add_circle</span>
                        </button>
                        <button
                            onClick={onLayout}
                            className="px-3 py-1.5 text-sm bg-muted hover:bg-muted/80 text-card-foreground rounded-lg flex items-center gap-1 transition-colors"
                        >
                            <span className="material-symbols-outlined text-[18px]">auto_fix_high</span>
                            Auto Layout
                        </button>
                        <button
                            onClick={handleDiscard}
                            disabled={!isManualEditMode || !hasUnsavedChanges || saving}
                            className="px-3 py-1.5 text-sm bg-muted hover:bg-muted/80 disabled:opacity-50 disabled:cursor-not-allowed text-card-foreground rounded-lg flex items-center gap-1 transition-colors"
                        >
                            <span className="material-symbols-outlined text-[18px]">restart_alt</span>
                            Discard
                        </button>
                        <button
                            onClick={handleSave}
                            disabled={!isManualEditMode || saving}
                            className="px-3 py-1.5 text-sm bg-primary hover:bg-primary/90 disabled:opacity-60 disabled:cursor-not-allowed text-primary-foreground rounded-lg flex items-center gap-1 transition-colors"
                        >
                            <span className="material-symbols-outlined text-[18px]">save</span>
                            {saving ? 'Saving...' : 'Save'}
                        </button>
                    </div>
                </div>

                {notice && (
                    <div
                        className={`px-4 py-2 text-sm border-b border-border ${
                            notice.type === 'success'
                                ? 'bg-green-500/10 text-green-400'
                                : notice.type === 'error'
                                    ? 'bg-red-500/10 text-red-400'
                                    : 'bg-amber-500/10 text-amber-400'
                        }`}
                    >
                        {notice.message}
                    </div>
                )}

                <div className="h-[550px]">
                    <ReactFlow
                        nodes={nodes}
                        edges={edges}
                        onNodesChange={handleNodesChange}
                        onEdgesChange={handleEdgesChange}
                        onConnect={onConnect}
                        nodeTypes={nodeTypes}
                        fitView
                        fitViewOptions={{ padding: 0.2 }}
                        className="bg-muted/30"
                        nodesDraggable
                        nodesConnectable={isManualEditMode}
                        elementsSelectable={isManualEditMode}
                    >
                        <Background color="#94a3b8" gap={20} size={1} />
                        <Controls className="!bg-card !border-border !rounded-lg !shadow-lg" />
                        <MiniMap
                            className="!bg-card !border-border !rounded-lg !w-[120px] !h-[90px] !min-w-[120px] !min-h-[90px] [&>svg]:!w-full [&>svg]:!h-full"
                            nodeColor={(node) => {
                                const nodeType = normalizeNodeType(isRecord(node.data) ? node.data.nodeType : undefined);
                                const colors: Record<ArchitectureNodeType, string> = {
                                    api: '#3b82f6',
                                    gateway: '#3b82f6',
                                    database: '#f59e0b',
                                    cache: '#ef4444',
                                    queue: '#a855f7',
                                    storage: '#22c55e',
                                    frontend: '#06b6d4',
                                    worker: '#f97316',
                                    auth: '#ec4899',
                                    service: '#6366f1',
                                    client: '#64748b',
                                    mobile: '#64748b',
                                };
                                return colors[nodeType];
                            }}
                        />
                    </ReactFlow>
                </div>
            </div>

            <div className="bg-card border border-border rounded-xl p-4">
                <h4 className="text-xs font-bold text-muted-foreground uppercase tracking-wider mb-3">Component Types</h4>
                <div className="flex flex-wrap gap-3">
                    {Object.entries(typeIcons).map(([type, icon]) => {
                        const normalizedType = normalizeNodeType(type);
                        const colors = typeColors[normalizedType];
                        return (
                            <div key={type} className="flex items-center gap-1.5 text-xs">
                                <span className={`material-symbols-outlined text-sm ${colors.text}`}>{icon}</span>
                                <span className="text-card-foreground capitalize">{type}</span>
                            </div>
                        );
                    })}
                </div>
            </div>

            {showAddComponentModal && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                    <div className="bg-card rounded-xl shadow-2xl w-full max-w-lg mx-4 border border-border">
                        <div className="flex justify-between items-center px-6 py-4 border-b border-border">
                            <h3 className="font-bold text-lg text-card-foreground flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">add_circle</span>
                                Add Component
                            </h3>
                            <button
                                onClick={() => setShowAddComponentModal(false)}
                                className="text-muted-foreground hover:text-card-foreground"
                            >
                                <span className="material-symbols-outlined">close</span>
                            </button>
                        </div>
                        <div className="p-6 space-y-4">
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-2">
                                    Label
                                </label>
                                <input
                                    value={addComponentDraft.label}
                                    onChange={(event) =>
                                        setAddComponentDraft((prev) => ({ ...prev, label: event.target.value }))
                                    }
                                    placeholder="e.g. Redis Cache"
                                    className="w-full px-3 py-2 bg-muted border border-border rounded-lg text-sm text-card-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary"
                                />
                            </div>
                            <div>
                                <label className="block text-sm font-medium text-card-foreground mb-2">
                                    Component Type
                                </label>
                                <select
                                    value={addComponentDraft.type}
                                    onChange={(event) =>
                                        setAddComponentDraft((prev) => ({
                                            ...prev,
                                            type: normalizeNodeType(event.target.value),
                                        }))
                                    }
                                    className="w-full px-3 py-2 bg-muted border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary"
                                >
                                    {NODE_TYPES.map((nodeType) => (
                                        <option key={nodeType} value={nodeType}>
                                            {nodeType}
                                        </option>
                                    ))}
                                </select>
                            </div>
                            <div className="flex justify-end gap-3">
                                <button
                                    onClick={() => setShowAddComponentModal(false)}
                                    className="px-4 py-2 text-sm bg-muted hover:bg-muted/80 text-card-foreground rounded-lg transition-colors"
                                >
                                    Cancel
                                </button>
                                <button
                                    onClick={handleAddComponent}
                                    className="px-4 py-2 text-sm bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg transition-colors"
                                >
                                    Add
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}

            {pendingSync && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
                    <div className="bg-card rounded-xl shadow-2xl w-full max-w-2xl mx-4 border border-border">
                        <div className="flex justify-between items-center px-6 py-4 border-b border-border">
                            <h3 className="font-bold text-lg text-card-foreground flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">sync_alt</span>
                                Architecture Changed
                            </h3>
                            <button
                                onClick={() => setPendingSync(null)}
                                className="text-muted-foreground hover:text-card-foreground"
                            >
                                <span className="material-symbols-outlined">close</span>
                            </button>
                        </div>
                        <div className="p-6 space-y-4">
                            <p className="text-sm text-muted-foreground">
                                Architecture was saved. Do you want to create a task for agent to apply these changes to codebase?
                            </p>
                            <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                                <div className="bg-muted/40 border border-border rounded-lg p-3">
                                    <div className="font-semibold text-card-foreground mb-1">Added Nodes</div>
                                    <div className="text-muted-foreground">{pendingSync.diff.addedNodes.join(', ') || 'None'}</div>
                                </div>
                                <div className="bg-muted/40 border border-border rounded-lg p-3">
                                    <div className="font-semibold text-card-foreground mb-1">Removed Nodes</div>
                                    <div className="text-muted-foreground">{pendingSync.diff.removedNodes.join(', ') || 'None'}</div>
                                </div>
                                <div className="bg-muted/40 border border-border rounded-lg p-3">
                                    <div className="font-semibold text-card-foreground mb-1">Changed Nodes</div>
                                    <div className="text-muted-foreground">{pendingSync.diff.changedNodes.join(', ') || 'None'}</div>
                                </div>
                                <div className="bg-muted/40 border border-border rounded-lg p-3">
                                    <div className="font-semibold text-card-foreground mb-1">Added Edges</div>
                                    <div className="text-muted-foreground">{pendingSync.diff.addedEdges.join(', ') || 'None'}</div>
                                </div>
                            </div>
                            <div className="flex justify-end gap-3">
                                <button
                                    onClick={() => setPendingSync(null)}
                                    disabled={creatingSyncTask}
                                    className="px-4 py-2 text-sm bg-muted hover:bg-muted/80 text-card-foreground rounded-lg transition-colors disabled:opacity-60"
                                >
                                    Skip
                                </button>
                                <button
                                    onClick={handleCreateSyncTask}
                                    disabled={creatingSyncTask}
                                    className="px-4 py-2 text-sm bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg transition-colors disabled:opacity-60"
                                >
                                    {creatingSyncTask ? 'Creating Task...' : 'Create Task'}
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            )}
        </div>
    );
}
