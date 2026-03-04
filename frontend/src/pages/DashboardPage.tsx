// DashboardPage - Refactored with components and custom hook
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { AppShell } from '../components/layout/AppShell';
import { CreateTaskModal } from '../components/modals';
import { useDashboard } from '../hooks/useDashboard';
import type { DashboardHumanTaskDoc } from '../api/generated/models/dashboardHumanTaskDoc';
import {
    StatCard,
    ProjectsTable,
    AgentFeed,
    HumanTaskCard
} from '../components/dashboard';

// Loading skeleton component
function DashboardSkeleton() {
    return (
        <div className="animate-pulse">
            {/* Stats skeleton */}
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
                {[1, 2, 3, 4].map((i) => (
                    <div key={i} className="p-5 rounded-xl bg-card border border-border h-32">
                        <div className="h-10 w-10 bg-muted rounded-lg mb-4"></div>
                        <div className="h-4 w-24 bg-muted rounded mb-2"></div>
                        <div className="h-8 w-16 bg-muted rounded"></div>
                    </div>
                ))}
            </div>

            {/* Content skeleton */}
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
                <div className="lg:col-span-8 space-y-6">
                    <div className="h-64 bg-muted rounded-xl"></div>
                    <div className="h-80 bg-muted rounded-xl"></div>
                </div>
                <div className="lg:col-span-4">
                    <div className="h-[500px] bg-muted rounded-xl"></div>
                </div>
            </div>
        </div>
    );
}

export function DashboardPage() {
    const navigate = useNavigate();
    const { stats, projects, humanTasks, loading, error } = useDashboard();
    const [showCreateTaskModal, setShowCreateTaskModal] = useState(false);

    const getTaskRoute = (task: DashboardHumanTaskDoc, preferLatestAttempt: boolean): string => {
        if (task.projectId) {
            if (preferLatestAttempt) {
                return `/tasks/projects/${task.projectId}/${task.id}/attempts/latest`;
            }
            return `/tasks/projects/${task.projectId}/${task.id}`;
        }
        return `/tasks?taskId=${task.id}`;
    };

    const handleTaskReview = (task: DashboardHumanTaskDoc) => {
        navigate(getTaskRoute(task, true));
    };

    const handleTaskApprove = (task: DashboardHumanTaskDoc) => {
        navigate(getTaskRoute(task, false));
    };

    const handleTaskAssign = (task: DashboardHumanTaskDoc) => {
        navigate(getTaskRoute(task, false));
    };

    const handleAddTask = () => {
        setShowCreateTaskModal(true);
    };

    if (loading) {
        return (
            <AppShell>
                <div className="flex-1 overflow-y-auto p-6 md:p-8">
                    <div className="max-w-[1600px] mx-auto">
                        <DashboardSkeleton />
                    </div>
                </div>
            </AppShell>
        );
    }

    if (error) {
        return (
            <AppShell>
                <div className="flex-1 overflow-y-auto p-6 md:p-8">
                    <div className="max-w-[1600px] mx-auto">
                        <div className="bg-red-100 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg">
                            {error}
                        </div>
                    </div>
                </div>
            </AppShell>
        );
    }

    const urgentTaskCount = humanTasks?.filter(t => t.type === 'blocker').length || 0;

    return (
        <AppShell>
            <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide">
                <div className="max-w-[1600px] mx-auto flex flex-col gap-6">
                    {/* Stats Row */}
                    <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                        <StatCard
                            icon="folder_open"
                            iconBgColor="bg-blue-100 dark:bg-blue-500/20"
                            iconTextColor="text-blue-600 dark:text-blue-400"
                            label="Active Projects"
                            value={stats?.activeProjects?.count || 0}
                            badge={{ text: stats?.activeProjects?.trend || '', variant: 'success' }}
                        />
                        <StatCard
                            icon="smart_toy"
                            iconBgColor="bg-purple-100 dark:bg-purple-500/20"
                            iconTextColor="text-purple-600 dark:text-purple-400"
                            label="Agents Online"
                            value={
                                <>
                                    {stats?.agentsOnline?.online || 0}
                                    <span className="text-lg text-slate-400 font-normal">/{stats?.agentsOnline?.total || 0}</span>
                                </>
                            }
                            badge={{ text: 'Live', variant: 'success' }}
                        />
                        <StatCard
                            icon="memory"
                            iconBgColor="bg-orange-100 dark:bg-orange-500/20"
                            iconTextColor="text-orange-600 dark:text-orange-400"
                            label="System Load"
                            value={`${stats?.systemLoad?.percentage || 0}%`}
                            badge={{
                                text: stats?.systemLoad?.status === 'high' ? 'High Usage' : 'Normal',
                                variant: stats?.systemLoad?.status === 'high' ? 'danger' : 'warning'
                            }}
                            progress={{
                                value: stats?.systemLoad?.percentage || 0,
                                color: 'bg-orange-500'
                            }}
                        />
                        <StatCard
                            icon="commit"
                            iconBgColor="bg-pink-100 dark:bg-pink-500/20"
                            iconTextColor="text-pink-600 dark:text-pink-400"
                            label="Pending PRs"
                            value={stats?.pendingPrs?.count || 0}
                            badge={{
                                text: stats?.pendingPrs?.requiresReview ? 'Requires Review' : 'All Good',
                                variant: 'info'
                            }}
                        />
                    </div>

                    {/* Main Content Split */}
                    <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
                        {/* Projects Table Section */}
                        <div className="lg:col-span-8 flex flex-col gap-6">
                            {projects && projects.length > 0 && (
                                <ProjectsTable projects={projects} />
                            )}

                            {/* Agent Live Feed */}
                            <AgentFeed />
                        </div>

                        {/* Right Column: Tasks */}
                        <div className="lg:col-span-4 flex flex-col h-full">
                            <div className="rounded-xl bg-card border border-border overflow-hidden shadow-sm h-full flex flex-col">
                                <div className="px-6 py-5 border-b border-border flex justify-between items-center">
                                    <h3 className="font-bold text-lg text-card-foreground flex items-center gap-2">
                                        Tasks
                                        {urgentTaskCount > 0 && (
                                            <span className="bg-red-100 text-red-600 dark:bg-red-500/20 dark:text-red-400 text-xs font-bold px-2 py-0.5 rounded-full">
                                                {urgentTaskCount} Urgent
                                            </span>
                                        )}
                                    </h3>
                                    <button
                                        onClick={() => navigate('/tasks')}
                                        className="text-muted-foreground hover:text-primary transition-colors"
                                        title="View all tasks"
                                    >
                                        <span className="material-symbols-outlined">filter_list</span>
                                    </button>
                                </div>
                                <div className="flex-1 overflow-y-auto p-4 flex flex-col gap-3">
                                    {humanTasks?.map((task) => (
                                        <HumanTaskCard
                                            key={task.id}
                                            task={task}
                                            onReview={handleTaskReview}
                                            onApprove={handleTaskApprove}
                                            onAssign={handleTaskAssign}
                                        />
                                    ))}

                                    <button
                                        onClick={handleAddTask}
                                        className="mt-2 w-full py-2 border border-dashed border-border hover:border-primary rounded-lg text-sm text-muted-foreground hover:text-primary transition-colors flex items-center justify-center gap-2"
                                    >
                                        <span className="material-symbols-outlined text-sm">add</span>
                                        Add Task
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            {/* Create Task Modal */}
            {showCreateTaskModal && (
                <CreateTaskModal
                    isOpen={showCreateTaskModal}
                    onClose={() => setShowCreateTaskModal(false)}
                />
            )}
        </AppShell>
    );
}
