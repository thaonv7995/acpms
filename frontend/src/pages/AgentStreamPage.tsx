import { useEffect, useMemo, useState } from 'react';
import { AppShell } from '../components/layout/AppShell';
import { AgentStatusCard } from '../components/agent-stream';
import { TimelineLogDisplay } from '../components/timeline-log';
import { useAgentLogs } from '../hooks/useAgentLogs';
import { ReviewChangesModal } from '../components/modals/ReviewChangesModal';
import type { AgentStatus } from '../api/agentLogs';

type StatusFilter = 'all' | 'active' | 'completed';
type ViewMode = 'grid' | 'list';

const INITIAL_VISIBLE_COUNT = 4;
const RUNNING_TIMELINES_PER_PAGE = 4;

function sortStatuses(items: AgentStatus[]) {
    return [...items].sort((a, b) => {
        const statusOrder: Record<string, number> = {
            running: 0,
            queued: 1,
            success: 2,
            failed: 2,
            cancelled: 3,
        };
        const orderA = statusOrder[a.status] ?? 4;
        const orderB = statusOrder[b.status] ?? 4;
        if (orderA !== orderB) return orderA - orderB;
        return new Date(b.created_at).getTime() - new Date(a.created_at).getTime();
    });
}

function matchesSearch(status: AgentStatus, query: string) {
    if (!query) return true;
    const normalized = query.toLowerCase();
    return (
        status.task_title.toLowerCase().includes(normalized) ||
        status.project_name.toLowerCase().includes(normalized)
    );
}

export function AgentStreamPage() {
    const [searchQuery, setSearchQuery] = useState('');
    const [selectedAttempt, setSelectedAttempt] = useState<string | null>(null);
    const [statusFilter, setStatusFilter] = useState<StatusFilter>('active');
    const [viewMode, setViewMode] = useState<ViewMode>('grid');
    const [showAll, setShowAll] = useState(false);
    const [reviewModalOpen, setReviewModalOpen] = useState(false);
    const [reviewAttempt, setReviewAttempt] = useState<{ id: string; taskTitle?: string; projectName?: string } | null>(null);
    const [runningPage, setRunningPage] = useState(1);

    const { statuses, loading, error, refetch } = useAgentLogs();
    const searchValue = searchQuery.trim();

    const filteredStatuses = useMemo(() => {
        let filtered = statuses;

        if (statusFilter === 'active') {
            filtered = filtered.filter(s => s.status === 'running' || s.status === 'queued');
        } else if (statusFilter === 'completed') {
            filtered = filtered.filter(s => s.status === 'success' || s.status === 'failed' || s.status === 'cancelled');
        }

        filtered = filtered.filter(status => matchesSearch(status, searchValue));
        return sortStatuses(filtered);
    }, [searchValue, statusFilter, statuses]);

    const runningStatuses = useMemo(() => {
        const running = statuses.filter(status => status.status === 'running');
        const searched = running.filter(status => matchesSearch(status, searchValue));
        return sortStatuses(searched);
    }, [searchValue, statuses]);

    const runningTotalPages = Math.ceil(runningStatuses.length / RUNNING_TIMELINES_PER_PAGE) || 1;
    const paginatedRunningStatuses = useMemo(
        () => runningStatuses.slice(
            (runningPage - 1) * RUNNING_TIMELINES_PER_PAGE,
            runningPage * RUNNING_TIMELINES_PER_PAGE
        ),
        [runningStatuses, runningPage]
    );

    useEffect(() => {
        if (runningPage > runningTotalPages) setRunningPage(1);
    }, [runningPage, runningTotalPages]);

    const visibleStatuses = useMemo(() => {
        if (showAll || filteredStatuses.length <= INITIAL_VISIBLE_COUNT) {
            return filteredStatuses;
        }
        return filteredStatuses.slice(0, INITIAL_VISIBLE_COUNT);
    }, [filteredStatuses, showAll]);

    const hiddenCount = filteredStatuses.length - visibleStatuses.length;

    const selectedStatus = useMemo(
        () => (selectedAttempt ? statuses.find(status => status.id === selectedAttempt) ?? null : null),
        [selectedAttempt, statuses]
    );

    const handleAgentClick = (attemptId: string) => {
        if (selectedAttempt === attemptId) {
            setSelectedAttempt(null);
            return;
        }
        setSelectedAttempt(attemptId);
    };

    const handleReviewClick = (agent: { id: string; task_title: string; project_name: string }) => {
        setReviewAttempt({
            id: agent.id,
            taskTitle: agent.task_title,
            projectName: agent.project_name,
        });
        setReviewModalOpen(true);
    };

    const counts = useMemo(() => ({
        all: statuses.length,
        running: statuses.filter(s => s.status === 'running').length,
        queued: statuses.filter(s => s.status === 'queued').length,
        active: statuses.filter(s => s.status === 'running' || s.status === 'queued').length,
        completed: statuses.filter(s => s.status === 'success' || s.status === 'failed' || s.status === 'cancelled').length,
    }), [statuses]);

    if (loading) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="text-center">
                        <div className="animate-spin rounded-full h-12 w-12 border-b-2 border-primary mx-auto mb-4"></div>
                        <p className="text-muted-foreground">Loading agent logs...</p>
                    </div>
                </div>
            </AppShell>
        );
    }

    if (error) {
        return (
            <AppShell>
                <div className="flex-1 flex items-center justify-center">
                    <div className="text-center">
                        <span className="material-symbols-outlined text-red-500 dark:text-red-400 text-5xl mb-4">error</span>
                        <p className="text-red-500 dark:text-red-400 mb-2">Failed to load agent logs</p>
                        <p className="text-muted-foreground text-sm">{error}</p>
                        <button
                            onClick={refetch}
                            className="mt-4 px-4 py-2 bg-primary text-primary-foreground rounded-lg text-sm font-medium hover:bg-primary/90 transition-colors"
                        >
                            Retry
                        </button>
                    </div>
                </div>
            </AppShell>
        );
    }

    return (
        <AppShell>
            <div className="flex flex-1 overflow-hidden h-full">
                <main className="flex-1 flex flex-col min-w-0 bg-background relative">
                    <div className="px-6 pt-5 pb-3">
                        <div className="flex justify-between items-start">
                            <div>
                                <h1 className="text-2xl font-bold text-card-foreground">Agent Activity Stream</h1>
                                <p className="text-sm text-muted-foreground mt-1">
                                    Monitor running agent tasks and their logs
                                </p>
                            </div>
                            <div className="flex items-center gap-2">
                                {counts.running > 0 && (
                                    <span className="flex items-center gap-2 px-3 py-1 rounded-full bg-green-500/10 text-green-600 dark:text-green-400 text-xs font-bold border border-green-500/20">
                                        <span className="w-2 h-2 rounded-full bg-green-500 animate-pulse"></span>
                                        {counts.running} Running
                                    </span>
                                )}
                                {counts.queued > 0 && (
                                    <span className="flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 text-xs font-bold border border-blue-500/20">
                                        {counts.queued} Queued
                                    </span>
                                )}
                                <button
                                    onClick={refetch}
                                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-muted text-muted-foreground text-xs font-medium hover:bg-muted/80 transition-colors"
                                >
                                    <span className="material-symbols-outlined text-[16px]">refresh</span>
                                    Refresh
                                </button>
                            </div>
                        </div>
                    </div>

                    <div className="px-6 pb-3 flex items-center justify-between">
                        <div className="flex items-center gap-1 bg-muted rounded-lg p-1">
                            <button
                                onClick={() => { setStatusFilter('active'); setShowAll(false); }}
                                className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                                    statusFilter === 'active'
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-card-foreground hover:bg-muted/80'
                                }`}
                            >
                                Active {counts.active > 0 && `(${counts.active})`}
                            </button>
                            <button
                                onClick={() => { setStatusFilter('completed'); setShowAll(false); }}
                                className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                                    statusFilter === 'completed'
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-card-foreground hover:bg-muted/80'
                                }`}
                            >
                                Completed {counts.completed > 0 && `(${counts.completed})`}
                            </button>
                            <button
                                onClick={() => { setStatusFilter('all'); setShowAll(false); }}
                                className={`px-3 py-1.5 text-xs font-medium rounded-md transition-all ${
                                    statusFilter === 'all'
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-card-foreground hover:bg-muted/80'
                                }`}
                            >
                                All {counts.all > 0 && `(${counts.all})`}
                            </button>
                        </div>

                        <div className="flex items-center gap-0.5 bg-muted rounded-lg p-0.5">
                            <button
                                onClick={() => setViewMode('list')}
                                className={`h-8 w-8 flex items-center justify-center rounded-md transition-all ${
                                    viewMode === 'list'
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-card-foreground hover:bg-muted/80'
                                }`}
                                title="List view"
                            >
                                <span className="material-symbols-outlined text-[16px]">view_list</span>
                            </button>
                            <button
                                onClick={() => setViewMode('grid')}
                                className={`h-8 w-8 flex items-center justify-center rounded-md transition-all ${
                                    viewMode === 'grid'
                                        ? 'bg-primary text-primary-foreground shadow-sm'
                                        : 'text-muted-foreground hover:text-card-foreground hover:bg-muted/80'
                                }`}
                                title="Grid view"
                            >
                                <span className="material-symbols-outlined text-[16px]">grid_view</span>
                            </button>
                        </div>
                    </div>

                    {filteredStatuses.length > 0 ? (
                        <div className="px-6 pb-3">
                            {viewMode === 'grid' ? (
                                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-3">
                                    {visibleStatuses.map(agent => (
                                        <div
                                            key={agent.id}
                                            className={`transition-all rounded-xl ${selectedAttempt === agent.id ? 'ring-2 ring-primary ring-offset-2 ring-offset-background' : ''}`}
                                        >
                                            <AgentStatusCard
                                                agent={agent}
                                                onClick={() => handleAgentClick(agent.id)}
                                                onReviewClick={() => handleReviewClick(agent)}
                                            />
                                        </div>
                                    ))}
                                </div>
                            ) : (
                                <div className="flex flex-col gap-2">
                                    {visibleStatuses.map(agent => (
                                        <div
                                            key={agent.id}
                                            className={`transition-all rounded-lg ${selectedAttempt === agent.id ? 'ring-2 ring-primary ring-offset-1 ring-offset-background' : ''}`}
                                        >
                                            <AgentStatusCard
                                                agent={agent}
                                                onClick={() => handleAgentClick(agent.id)}
                                                onReviewClick={() => handleReviewClick(agent)}
                                                compact
                                            />
                                        </div>
                                    ))}
                                </div>
                            )}

                            {filteredStatuses.length > INITIAL_VISIBLE_COUNT && (
                                <button
                                    onClick={() => setShowAll(!showAll)}
                                    className="mt-2 w-full py-2 text-xs font-medium text-muted-foreground hover:text-card-foreground hover:bg-muted rounded-lg transition-colors flex items-center justify-center gap-1"
                                >
                                    <span className="material-symbols-outlined text-[16px]">
                                        {showAll ? 'expand_less' : 'expand_more'}
                                    </span>
                                    {showAll ? 'Show less' : `Show ${hiddenCount} more`}
                                </button>
                            )}
                        </div>
                    ) : (
                        <div className="px-6 pb-3">
                            <div className="bg-muted rounded-xl p-4 text-center">
                                <span className="material-symbols-outlined text-muted-foreground/50 text-3xl mb-1">
                                    {statusFilter === 'active' ? 'hourglass_empty' : 'check_circle'}
                                </span>
                                <p className="text-muted-foreground text-sm">
                                    {statusFilter === 'active'
                                        ? 'No active agents'
                                        : statusFilter === 'completed'
                                        ? 'No completed tasks yet'
                                        : 'No agent tasks'}
                                </p>
                            </div>
                        </div>
                    )}

                    <div className="flex-1 px-6 pb-6 min-h-0 flex flex-col">
                        <div className="flex flex-col h-full rounded-xl border border-border bg-background overflow-hidden">
                            <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-muted/20 gap-3">
                                <div className="relative w-full max-w-sm">
                                    <span className="material-symbols-outlined absolute left-2.5 top-2 text-muted-foreground text-[20px]">search</span>
                                    <input
                                        className="w-full bg-background border border-border text-sm text-foreground rounded-lg pl-9 pr-9 py-1.5 focus:outline-none focus:ring-1 focus:ring-primary focus:border-primary placeholder-muted-foreground"
                                        placeholder="Filter sessions..."
                                        type="text"
                                        value={searchQuery}
                                        onChange={(e) => setSearchQuery(e.target.value)}
                                    />
                                    {searchQuery && (
                                        <button
                                            onClick={() => setSearchQuery('')}
                                            className="absolute right-2.5 top-2 text-muted-foreground hover:text-foreground"
                                        >
                                            <span className="material-symbols-outlined text-[18px]">close</span>
                                        </button>
                                    )}
                                </div>
                                <span className="text-xs text-muted-foreground whitespace-nowrap">
                                    {selectedAttempt
                                        ? 'Focused attempt'
                                        : `${runningStatuses.length} running session${runningStatuses.length !== 1 ? 's' : ''}`}
                                </span>
                            </div>

                            {selectedAttempt && (
                                <div className="px-4 py-2.5 border-b border-dashed border-border bg-background flex items-center justify-between gap-3">
                                    <div className="min-w-0">
                                        <p className="text-sm font-medium text-foreground truncate" title={selectedStatus?.task_title ?? 'Selected attempt'}>
                                            {selectedStatus?.task_title ?? 'Selected attempt'}
                                        </p>
                                        {selectedStatus?.project_name && (
                                            <p className="text-xs text-muted-foreground truncate" title={selectedStatus.project_name}>
                                                {selectedStatus.project_name}
                                            </p>
                                        )}
                                    </div>
                                    <button
                                        onClick={() => setSelectedAttempt(null)}
                                        className="shrink-0 h-7 px-2.5 rounded-md border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
                                    >
                                        Back to running sessions
                                    </button>
                                </div>
                            )}

                            <div className="flex-1 min-h-0">
                                {selectedAttempt ? (
                                    <TimelineLogDisplay
                                        key={`selected-${selectedAttempt}`}
                                        attemptId={selectedAttempt}
                                        attemptStatus={selectedStatus?.status}
                                    />
                                ) : runningStatuses.length > 0 ? (
                                    <div className="h-full overflow-y-auto p-3 flex flex-col">
                                        <div className="grid grid-cols-1 xl:grid-cols-2 gap-3 flex-1">
                                            {paginatedRunningStatuses.map((status) => (
                                                <section
                                                    key={status.id}
                                                    className="rounded-lg border border-border bg-background overflow-hidden"
                                                >
                                                    <div className="px-3 py-2 border-b border-dashed border-border bg-muted/10 flex items-center justify-between gap-3">
                                                        <div className="min-w-0">
                                                            <p className="text-sm font-medium text-foreground truncate" title={status.task_title}>
                                                                {status.task_title}
                                                            </p>
                                                            <p className="text-xs text-muted-foreground truncate" title={status.project_name}>
                                                                {status.project_name}
                                                            </p>
                                                        </div>
                                                        <button
                                                            onClick={() => setSelectedAttempt(status.id)}
                                                            className="shrink-0 h-7 px-2.5 rounded-md border border-border text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
                                                        >
                                                            Focus
                                                        </button>
                                                    </div>
                                                    <div className="h-[340px] min-h-[340px]">
                                                        <TimelineLogDisplay
                                                            key={`running-${status.id}`}
                                                            attemptId={status.id}
                                                            attemptStatus={status.status}
                                                        />
                                                    </div>
                                                </section>
                                            ))}
                                        </div>
                                        {runningTotalPages > 1 && (
                                            <div className="flex items-center justify-center gap-2 mt-4 pt-3 border-t border-border">
                                                <button
                                                    onClick={() => setRunningPage(p => Math.max(1, p - 1))}
                                                    disabled={runningPage <= 1}
                                                    className="h-8 px-3 rounded-md border border-border text-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                                                >
                                                    Previous
                                                </button>
                                                <span className="text-sm text-muted-foreground">
                                                    Page {runningPage} / {runningTotalPages}
                                                </span>
                                                <button
                                                    onClick={() => setRunningPage(p => Math.min(runningTotalPages, p + 1))}
                                                    disabled={runningPage >= runningTotalPages}
                                                    className="h-8 px-3 rounded-md border border-border text-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
                                                >
                                                    Next
                                                </button>
                                            </div>
                                        )}
                                    </div>
                                ) : (
                                    <div className="flex flex-col items-center justify-center h-full px-6">
                                        <span className="material-symbols-outlined text-muted-foreground/50 text-5xl mb-3">hourglass_empty</span>
                                        <p className="text-muted-foreground text-sm font-medium">
                                            No running attempts
                                        </p>
                                        <p className="text-muted-foreground/80 text-xs mt-1 text-center max-w-md">
                                            Start a task to stream logs here, or select an attempt card to inspect its timeline.
                                        </p>
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>
                </main>
            </div>

            {reviewAttempt && (
                <ReviewChangesModal
                    isOpen={reviewModalOpen}
                    onClose={() => {
                        setReviewModalOpen(false);
                        setReviewAttempt(null);
                    }}
                    attemptId={reviewAttempt.id}
                    taskTitle={reviewAttempt.taskTitle}
                    projectName={reviewAttempt.projectName}
                    onApproved={refetch}
                />
            )}
        </AppShell>
    );
}
