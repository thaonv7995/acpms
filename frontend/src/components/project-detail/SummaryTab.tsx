// SummaryTab Component - Project overview with key information
import { useEffect, useMemo, useState } from 'react';
import type { Requirement } from '../../api/requirements';
import { createProjectSprint, getSprintOverview, type SprintOverview, type SprintWithRoadmapFields } from '../../api/sprints';
import type { ProjectMetadata } from '../../shared/types';
import { resolveTechStack } from '../../utils/resolveTechStack';
import { logger } from '@/lib/logger';

type SummaryNavigationTab = 'kanban' | 'requirements' | 'architecture';

interface SummaryTabProps {
    projectId: string;
    description?: string;
    repositoryUrl?: string;
    metadata?: ProjectMetadata;
    /** Raw project from API (metadata, architecture_config, project_type) for consistent tech stack resolution */
    rawProject?: { metadata?: unknown; architecture_config?: unknown; project_type?: string } | null;
    requirements: Requirement[];
    sprints: SprintWithRoadmapFields[];
    selectedSprintId: string | null;
    onSelectSprint: (sprintId: string | null) => void;
    onNavigateTab: (tab: SummaryNavigationTab) => void;
    onRefreshProject: () => Promise<void> | void;
    onRequirementClick?: (reqId: string) => void;
}

// Tech stack icon mapping (lowercase keys)
const techIcons: Record<string, string> = {
    react: 'code',
    'react + vite': 'code',
    vite: 'bolt',
    typescript: 'javascript',
    'node.js': 'terminal',
    nodejs: 'terminal',
    python: 'code',
    rust: 'memory',
    postgresql: 'database',
    mongodb: 'database',
    redis: 'memory',
    docker: 'deployed_code',
    kubernetes: 'cloud',
    aws: 'cloud',
    gcp: 'cloud',
    azure: 'cloud',
    tailwind: 'palette',
    'tailwind css': 'palette',
    nextjs: 'web',
    'next.js': 'web',
    tauri: 'desktop_windows',
    fastapi: 'api',
};

const sprintStatusStyles: Record<string, { bg: string; text: string; dot: string }> = {
    active: { bg: 'bg-green-100 dark:bg-green-500/20', text: 'text-green-600 dark:text-green-400', dot: 'bg-green-500' },
    planned: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', dot: 'bg-blue-500' },
    planning: { bg: 'bg-blue-100 dark:bg-blue-500/20', text: 'text-blue-600 dark:text-blue-400', dot: 'bg-blue-500' },
    closed: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/60' },
    completed: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/60' },
    archived: { bg: 'bg-muted', text: 'text-muted-foreground', dot: 'bg-muted-foreground/40' },
};

function normalizeSprintStatus(status: string | undefined | null): string {
    if (!status) return 'planned';
    const lower = status.toLowerCase();
    if (lower === 'planning') return 'planned';
    if (lower === 'completed') return 'closed';
    return lower;
}

function formatSprintDateRange(startDate?: string | null, endDate?: string | null): string {
    if (!startDate && !endDate) return 'No date range';

    const formatDate = (value?: string | null) => {
        if (!value) return '—';
        const parsed = new Date(value);
        if (Number.isNaN(parsed.getTime())) return '—';
        return parsed.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
    };

    return `${formatDate(startDate)} - ${formatDate(endDate)}`;
}

export function SummaryTab({
    projectId,
    description,
    repositoryUrl,
    metadata,
    rawProject,
    requirements,
    sprints,
    selectedSprintId,
    onSelectSprint,
    onNavigateTab,
    onRefreshProject,
    onRequirementClick,
}: SummaryTabProps) {
    const techStack = useMemo(
        () => (rawProject ? resolveTechStack(rawProject) : (metadata?.techStack || [])),
        [rawProject, metadata?.techStack],
    );
    const progress = metadata?.progress || 0;

    const [sprintOverview, setSprintOverview] = useState<SprintOverview | null>(null);
    const [overviewLoading, setOverviewLoading] = useState(false);
    const [overviewError, setOverviewError] = useState<string | null>(null);

    const [showCreateSprintModal, setShowCreateSprintModal] = useState(false);
    const [createSprintLoading, setCreateSprintLoading] = useState(false);
    const [createSprintError, setCreateSprintError] = useState<string | null>(null);
    const [newSprintName, setNewSprintName] = useState('');
    const [newSprintGoal, setNewSprintGoal] = useState('');
    const [newSprintStartDate, setNewSprintStartDate] = useState('');
    const [newSprintEndDate, setNewSprintEndDate] = useState('');

    const sortedSprints = useMemo(() => {
        return [...sprints].sort((a, b) => {
            const aSequence = typeof a.sequence === 'number' ? a.sequence : Number.MAX_SAFE_INTEGER;
            const bSequence = typeof b.sequence === 'number' ? b.sequence : Number.MAX_SAFE_INTEGER;
            if (aSequence !== bSequence) return aSequence - bSequence;
            return new Date(a.created_at).getTime() - new Date(b.created_at).getTime();
        });
    }, [sprints]);

    const activeSprint = useMemo(
        () => sortedSprints.find((sprint) => normalizeSprintStatus(sprint.status) === 'active') || null,
        [sortedSprints],
    );

    const selectedSprint = useMemo(
        () => sortedSprints.find((sprint) => sprint.id === selectedSprintId) || null,
        [sortedSprints, selectedSprintId],
    );

    const overviewSprint = selectedSprint || activeSprint || sortedSprints[0] || null;

    const nextSprintSequence = useMemo(() => {
        const maxSequence = sortedSprints.reduce((acc, sprint) => {
            if (typeof sprint.sequence === 'number') {
                return Math.max(acc, sprint.sequence);
            }
            return acc;
        }, 0);
        return maxSequence + 1;
    }, [sortedSprints]);

    useEffect(() => {
        if (!overviewSprint) {
            setSprintOverview(null);
            setOverviewError(null);
            return;
        }

        let active = true;
        setOverviewLoading(true);
        setOverviewError(null);

        getSprintOverview(projectId, overviewSprint.id)
            .then((overview) => {
                if (!active) return;
                setSprintOverview(overview);
            })
            .catch((error) => {
                if (!active) return;
                logger.error('Failed to fetch sprint overview:', error);
                setSprintOverview(null);
                setOverviewError('Failed to load sprint overview');
            })
            .finally(() => {
                if (!active) return;
                setOverviewLoading(false);
            });

        return () => {
            active = false;
        };
    }, [overviewSprint?.id, projectId]);

    useEffect(() => {
        if (!showCreateSprintModal) return;
        setCreateSprintError(null);
        setNewSprintName(`Sprint ${nextSprintSequence}`);
        setNewSprintGoal('');
        setNewSprintStartDate('');
        setNewSprintEndDate('');
    }, [nextSprintSequence, showCreateSprintModal]);

    const handleCreateSprint = async () => {
        if (!newSprintName.trim()) {
            setCreateSprintError('Sprint name is required');
            return;
        }

        if (newSprintStartDate && newSprintEndDate && new Date(newSprintStartDate) > new Date(newSprintEndDate)) {
            setCreateSprintError('End date must be after start date');
            return;
        }

        setCreateSprintLoading(true);
        setCreateSprintError(null);

        try {
            const created = await createProjectSprint(projectId, {
                name: newSprintName.trim(),
                goal: newSprintGoal.trim() || undefined,
                sequence: nextSprintSequence,
                start_date: newSprintStartDate ? new Date(newSprintStartDate).toISOString() : undefined,
                end_date: newSprintEndDate ? new Date(newSprintEndDate).toISOString() : undefined,
            });

            await onRefreshProject();
            onSelectSprint(created.id);
            setShowCreateSprintModal(false);
        } catch (error) {
            logger.error('Failed to create sprint:', error);
            setCreateSprintError(error instanceof Error ? error.message : 'Failed to create sprint');
        } finally {
            setCreateSprintLoading(false);
        }
    };

    return (
        <>
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
                {/* Left Column - Main Info */}
                <div className="lg:col-span-2 space-y-6">
                    {/* Project Overview */}
                    <div className="bg-card border border-border rounded-xl p-6">
                        <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                            <span className="material-symbols-outlined text-primary">info</span>
                            Project Overview
                        </h3>

                        <div className="space-y-4">
                            {/* Description */}
                            <div>
                                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Description</label>
                                <p className="mt-1 text-card-foreground">
                                    {description || 'No description provided'}
                                </p>
                            </div>

                            {/* Repository */}
                            {repositoryUrl && (
                                <div>
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Repository</label>
                                    <a
                                        href={repositoryUrl.startsWith('http') ? repositoryUrl : `https://${repositoryUrl}`}
                                        target="_blank"
                                        rel="noopener noreferrer"
                                        className="mt-1 flex items-start gap-2 text-sm font-medium text-sky-400 hover:text-sky-300 hover:underline underline-offset-2 transition-colors"
                                    >
                                        <span className="material-symbols-outlined text-[16px] mt-0.5">link</span>
                                        <span className="break-all">{repositoryUrl}</span>
                                    </a>
                                </div>
                            )}

                            {/* Progress */}
                            <div>
                                <div className="flex justify-between items-center mb-2">
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Overall Progress</label>
                                    <span className="text-sm font-bold text-primary">{progress}%</span>
                                </div>
                                <div className="h-2 bg-muted rounded-full overflow-hidden">
                                    <div
                                        className="h-full bg-primary rounded-full transition-all duration-500"
                                        style={{ width: `${progress}%` }}
                                    />
                                </div>
                            </div>
                        </div>
                    </div>

                    {/* Tech Stack */}
                    <div className="bg-card border border-border rounded-xl p-6">
                        <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                            <span className="material-symbols-outlined text-purple-500">code</span>
                            Tech Stack
                        </h3>

                        {techStack.length > 0 ? (
                            <div className="flex flex-wrap gap-2">
                                {techStack.map((tech) => (
                                    <span
                                        key={tech}
                                        className="px-3 py-1.5 bg-muted text-card-foreground rounded-lg text-sm font-medium flex items-center gap-1.5"
                                    >
                                        <span className="material-symbols-outlined text-[16px] text-muted-foreground">
                                            {techIcons[tech.toLowerCase()] || 'code'}
                                        </span>
                                        {tech}
                                    </span>
                                ))}
                            </div>
                        ) : (
                            <div className="text-center py-8">
                                <span className="material-symbols-outlined text-4xl text-muted-foreground/50 mb-2">code_off</span>
                                <p className="text-muted-foreground text-sm">No tech stack defined</p>
                                <p className="text-muted-foreground/70 text-xs mt-1">Add technologies in project settings</p>
                            </div>
                        )}
                    </div>

                    {/* Requirements Summary */}
                    <div className="bg-card border border-border rounded-xl p-6">
                        <div className="flex justify-between items-center mb-4">
                            <h3 className="text-lg font-bold text-card-foreground flex items-center gap-2">
                                <span className="material-symbols-outlined text-amber-500">checklist</span>
                                Requirements
                            </h3>
                            <span className="text-sm text-muted-foreground">{requirements.length} total</span>
                        </div>

                        {requirements.length > 0 ? (
                            <div className="space-y-3">
                                {requirements.slice(0, 5).map((req) => (
                                    <button
                                        key={req.id}
                                        onClick={() => onRequirementClick?.(req.id)}
                                        className="w-full text-left p-4 bg-transparent border border-border rounded-xl hover:bg-muted/50 transition-colors focus:outline-none focus:ring-2 focus:ring-primary/20"
                                    >
                                        <div className="flex items-start gap-4">
                                            <div className="mt-1 flex-shrink-0">
                                                {req.status === 'in_progress' ? (
                                                    <span className="inline-block w-4 h-4 rounded-full border-2 border-blue-500/35 border-t-blue-500" />
                                                ) : req.status === 'done' ? (
                                                    <span className="material-symbols-outlined text-lg text-green-500">check_circle</span>
                                                ) : (
                                                    <span className="inline-block w-4 h-4 rounded-full border-2 border-muted-foreground" />
                                                )}
                                            </div>
                                            <div className="flex-1 min-w-0">
                                                <p className="text-base font-bold text-card-foreground truncate mb-2 leading-tight">{req.title}</p>
                                                <div className="flex items-center gap-3">
                                                    <span className={`text-[11px] font-bold px-2 py-0.5 rounded capitalize ${req.priority === 'critical' ? 'bg-red-500/10 text-red-500' :
                                                        req.priority === 'high' ? 'bg-orange-500/10 text-orange-500' :
                                                            req.priority === 'medium' ? 'bg-blue-500/10 text-blue-500' :
                                                                'bg-muted text-muted-foreground'
                                                        }`}>
                                                        {req.priority}
                                                    </span>
                                                    <span className="text-xs font-medium text-muted-foreground whitespace-nowrap">
                                                        {req.status === 'in_progress' ? 'In Progress' : req.status.charAt(0).toUpperCase() + req.status.slice(1)}
                                                    </span>
                                                </div>
                                            </div>
                                        </div>
                                    </button>
                                ))}
                                {requirements.length > 5 && (
                                    <p className="text-center text-sm text-muted-foreground">
                                        +{requirements.length - 5} more requirements
                                    </p>
                                )}
                            </div>
                        ) : (
                            <div className="text-center py-8">
                                <span className="material-symbols-outlined text-4xl text-muted-foreground/50 mb-2">playlist_add</span>
                                <p className="text-muted-foreground text-sm">No requirements defined</p>
                                <p className="text-muted-foreground/70 text-xs mt-1">Add requirements in the Requirements tab</p>
                            </div>
                        )}
                    </div>
                </div>

                {/* Right Column - Stats */}
                <div className="space-y-6">
                    {/* Sprint Overview */}
                    <div className="bg-card border border-border rounded-xl p-6">
                        <div className="flex items-center justify-between gap-3 mb-4">
                            <h3 className="text-lg font-bold text-card-foreground flex items-center gap-2">
                                <span className="material-symbols-outlined text-primary">sprint</span>
                                Sprint Overview
                            </h3>
                            <button
                                onClick={() => setShowCreateSprintModal(true)}
                                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-lg border border-border bg-muted hover:bg-muted/70 text-card-foreground transition-colors"
                            >
                                <span className="material-symbols-outlined text-[16px]">add</span>
                                Create Sprint
                            </button>
                        </div>

                        {overviewSprint ? (
                            <div className="space-y-4">
                                <div className="p-3 rounded-lg bg-muted/50 border border-border">
                                    <div className="flex items-start justify-between gap-2">
                                        <div>
                                            <p className="text-sm font-semibold text-card-foreground">{overviewSprint.name}</p>
                                            <p className="text-xs text-muted-foreground mt-1">
                                                {formatSprintDateRange(overviewSprint.start_date, overviewSprint.end_date)}
                                            </p>
                                        </div>
                                        <span
                                            className={`inline-flex items-center gap-1 px-2 py-1 rounded text-[10px] font-semibold uppercase ${(sprintStatusStyles[normalizeSprintStatus(overviewSprint.status)] || sprintStatusStyles.planned).bg
                                                } ${(sprintStatusStyles[normalizeSprintStatus(overviewSprint.status)] || sprintStatusStyles.planned).text}`}
                                        >
                                            <span className={`size-1.5 rounded-full ${(sprintStatusStyles[normalizeSprintStatus(overviewSprint.status)] || sprintStatusStyles.planned).dot}`}></span>
                                            {normalizeSprintStatus(overviewSprint.status)}
                                        </span>
                                    </div>
                                    {overviewSprint.goal && (
                                        <p className="text-xs text-card-foreground/80 mt-2">{overviewSprint.goal}</p>
                                    )}
                                </div>

                                {overviewLoading ? (
                                    <div className="grid grid-cols-2 gap-2 animate-pulse">
                                        <div className="h-16 rounded-lg bg-muted" />
                                        <div className="h-16 rounded-lg bg-muted" />
                                        <div className="h-16 rounded-lg bg-muted" />
                                        <div className="h-16 rounded-lg bg-muted" />
                                    </div>
                                ) : overviewError ? (
                                    <p className="text-sm text-red-500 dark:text-red-400">{overviewError}</p>
                                ) : sprintOverview ? (
                                    <>
                                        <div className="grid grid-cols-2 gap-2">
                                            <div className="rounded-lg border border-border p-2.5 bg-muted/40">
                                                <p className="text-[11px] text-muted-foreground uppercase">Completion</p>
                                                <p className="text-lg font-bold text-card-foreground">{sprintOverview.completionRate}%</p>
                                            </div>
                                            <div className="rounded-lg border border-border p-2.5 bg-muted/40">
                                                <p className="text-[11px] text-muted-foreground uppercase">Done</p>
                                                <p className="text-lg font-bold text-card-foreground">{sprintOverview.doneTasks}/{sprintOverview.totalTasks}</p>
                                            </div>
                                            <div className="rounded-lg border border-border p-2.5 bg-muted/40">
                                                <p className="text-[11px] text-muted-foreground uppercase">Remaining</p>
                                                <p className="text-lg font-bold text-card-foreground">{sprintOverview.remainingTasks}</p>
                                            </div>
                                            <div className="rounded-lg border border-border p-2.5 bg-muted/40">
                                                <p className="text-[11px] text-muted-foreground uppercase">Carry-over</p>
                                                <p className="text-lg font-bold text-card-foreground">{sprintOverview.movedOutCount}</p>
                                            </div>
                                        </div>

                                        <div>
                                            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">Sprint Roadmap</p>
                                            <div className="space-y-1.5 max-h-40 overflow-y-auto pr-1">
                                                {sortedSprints.map((sprint) => {
                                                    const status = normalizeSprintStatus(sprint.status);
                                                    const isSelected = overviewSprint.id === sprint.id;
                                                    const sequence = typeof sprint.sequence === 'number' ? sprint.sequence : '—';
                                                    const style = sprintStatusStyles[status] || sprintStatusStyles.planned;
                                                    return (
                                                        <button
                                                            key={sprint.id}
                                                            onClick={() => onSelectSprint(sprint.id)}
                                                            className={`w-full flex items-center justify-between gap-2 rounded-lg border px-2.5 py-2 text-left transition-colors ${isSelected
                                                                ? 'border-primary/50 bg-primary/5'
                                                                : 'border-border hover:bg-muted/60'
                                                                }`}
                                                        >
                                                            <div className="min-w-0">
                                                                <p className="text-sm font-medium text-card-foreground truncate">#{sequence} {sprint.name}</p>
                                                                <p className="text-[11px] text-muted-foreground truncate">{formatSprintDateRange(sprint.start_date, sprint.end_date)}</p>
                                                            </div>
                                                            <span className={`text-[10px] px-1.5 py-0.5 rounded uppercase font-semibold ${style.bg} ${style.text}`}>
                                                                {status}
                                                            </span>
                                                        </button>
                                                    );
                                                })}
                                            </div>
                                        </div>
                                    </>
                                ) : (
                                    <p className="text-sm text-muted-foreground">No sprint metrics available yet.</p>
                                )}
                            </div>
                        ) : (
                            <div className="text-center py-8">
                                <span className="material-symbols-outlined text-4xl text-muted-foreground/50 mb-2">sprint</span>
                                <p className="text-muted-foreground text-sm">No sprint available</p>
                                <p className="text-muted-foreground/70 text-xs mt-1">Create your first sprint to start planning.</p>
                            </div>
                        )}
                    </div>

                    {/* Quick Actions */}
                    <div className="bg-card border border-border rounded-xl p-6">
                        <h3 className="text-lg font-bold text-card-foreground mb-4 flex items-center gap-2">
                            <span className="material-symbols-outlined text-green-500">bolt</span>
                            Quick Links
                        </h3>

                        <div className="space-y-2">
                            <button
                                type="button"
                                onClick={() => onNavigateTab('kanban')}
                                className="flex items-center gap-3 p-2.5 rounded-lg hover:bg-muted transition-colors text-sm text-sky-400 hover:text-sky-300"
                            >
                                <span className="material-symbols-outlined text-sky-500">view_kanban</span>
                                <span>View Kanban Board</span>
                            </button>
                            <button
                                type="button"
                                onClick={() => onNavigateTab('requirements')}
                                className="flex items-center gap-3 p-2.5 rounded-lg hover:bg-muted transition-colors text-sm text-sky-400 hover:text-sky-300"
                            >
                                <span className="material-symbols-outlined text-sky-500">checklist</span>
                                <span>Manage Requirements</span>
                            </button>
                            <button
                                type="button"
                                onClick={() => onNavigateTab('architecture')}
                                className="flex items-center gap-3 p-2.5 rounded-lg hover:bg-muted transition-colors text-sm text-sky-400 hover:text-sky-300"
                            >
                                <span className="material-symbols-outlined text-sky-500">hub</span>
                                <span>View Architecture</span>
                            </button>
                        </div>
                    </div>
                </div>
            </div>

            {/* Create Sprint Modal */}
            {showCreateSprintModal && (
                <div className="fixed inset-0 z-[100] flex items-center justify-center p-4">
                    <div
                        className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
                        onClick={() => !createSprintLoading && setShowCreateSprintModal(false)}
                    />
                    <div className="relative w-full max-w-lg bg-card border border-border rounded-xl shadow-2xl p-6 space-y-4">
                        <div className="flex items-start justify-between gap-3">
                            <div>
                                <h4 className="text-lg font-bold text-card-foreground">Create Sprint</h4>
                                <p className="text-sm text-muted-foreground">Plan upcoming sprint without affecting active sprint.</p>
                            </div>
                            <button
                                onClick={() => setShowCreateSprintModal(false)}
                                disabled={createSprintLoading}
                                className="text-muted-foreground hover:text-card-foreground"
                            >
                                <span className="material-symbols-outlined">close</span>
                            </button>
                        </div>

                        <div className="space-y-3">
                            <div>
                                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Sprint Name</label>
                                <input
                                    value={newSprintName}
                                    onChange={(event) => setNewSprintName(event.target.value)}
                                    placeholder={`Sprint ${nextSprintSequence}`}
                                    className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                />
                            </div>

                            <div>
                                <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Goal</label>
                                <textarea
                                    value={newSprintGoal}
                                    onChange={(event) => setNewSprintGoal(event.target.value)}
                                    rows={3}
                                    placeholder="Describe sprint goal"
                                    className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 resize-none"
                                />
                            </div>

                            <div className="grid grid-cols-2 gap-3">
                                <div>
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Start Date</label>
                                    <input
                                        type="datetime-local"
                                        value={newSprintStartDate}
                                        onChange={(event) => setNewSprintStartDate(event.target.value)}
                                        className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                    />
                                </div>
                                <div>
                                    <label className="text-xs font-medium text-muted-foreground uppercase tracking-wider">End Date</label>
                                    <input
                                        type="datetime-local"
                                        value={newSprintEndDate}
                                        onChange={(event) => setNewSprintEndDate(event.target.value)}
                                        className="mt-1 w-full px-3 py-2 bg-card border border-border rounded-lg text-sm text-card-foreground focus:outline-none focus:ring-2 focus:ring-primary/20"
                                    />
                                </div>
                            </div>

                            {createSprintError && (
                                <p className="text-sm text-red-500 dark:text-red-400">{createSprintError}</p>
                            )}
                        </div>

                        <div className="flex justify-end gap-2 pt-2">
                            <button
                                onClick={() => setShowCreateSprintModal(false)}
                                disabled={createSprintLoading}
                                className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors"
                            >
                                Cancel
                            </button>
                            <button
                                onClick={handleCreateSprint}
                                disabled={createSprintLoading}
                                className="px-4 py-2 rounded-lg bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-semibold transition-colors disabled:opacity-60"
                            >
                                {createSprintLoading ? 'Creating...' : 'Create Sprint'}
                            </button>
                        </div>
                    </div>
                </div>
            )}
        </>
    );
}
