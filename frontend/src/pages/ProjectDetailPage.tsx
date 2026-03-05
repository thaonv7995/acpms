// ProjectDetailPage - Refactored with hooks and tab components
import { useState, useMemo, useEffect } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { AppShell } from '../components/layout/AppShell';
import {
  CreateTaskModal,
  ViewLogsModal,
  RequirementFormModal,
  RequirementDetailModal,
  RequirementBreakdownModal,
} from '../components/modals';
import { FloatingChatButton, ProjectAssistantPanel } from '../components/project-assistant';
import { getTaskAttempts } from '../api/taskAttempts';
import { createProjectFork, linkExistingFork, recheckProjectRepositoryAccess } from '../api/projects';
import { updateRequirement } from '../api/requirements';
import { getCurrentUser, isSystemAdmin } from '../api/auth';
import type { KanbanTask } from '../types/project';
import { useProjectDetail, ProjectTab } from '../hooks/useProjectDetail';
import { useProjectMembers } from '../hooks/useProjectMembers';
import { useProjectAssistant } from '../hooks/useProjectAssistant';
import {
  SummaryTab,
  TaskListTab,
  RequirementsTab,
  ArchitectureTab,
  DeploymentsTab,
  SettingsTab,
  SprintSelector,
} from '../components/project-detail';
import { ErrorBoundary } from '../components/common/ErrorBoundary';
import {
  getRepositoryAccessSummary,
  getRepositoryAccessTone,
  getRepositoryHref,
  getRepositoryModeLabel,
  normalizeRepositoryContext,
} from '../utils/repositoryAccess';
import { logger } from '@/lib/logger';

// Loading skeleton
function ProjectDetailSkeleton() {
  return (
    <div className="animate-pulse flex flex-col gap-6">
      {/* Header skeleton */}
      <div className="flex justify-between items-start">
        <div className="flex items-center gap-4">
          <div className="h-6 w-6 bg-muted rounded"></div>
          <div className="h-8 w-64 bg-muted rounded"></div>
        </div>
        <div className="flex gap-2">
          <div className="h-10 w-32 bg-muted rounded-lg"></div>
        </div>
      </div>
      {/* Stats skeleton */}
      <div className="grid grid-cols-4 gap-4">
        {[1, 2, 3, 4].map((i) => (
          <div key={i} className="h-24 bg-muted rounded-xl"></div>
        ))}
      </div>
      {/* Content skeleton */}
      <div className="h-[500px] bg-muted rounded-xl"></div>
    </div>
  );
}

const tabs: { id: ProjectTab; label: string; icon: string }[] = [
  { id: 'summary', label: 'Summary', icon: 'info' },
  { id: 'kanban', label: 'Tasks', icon: 'checklist' },
  { id: 'requirements', label: 'Requirements', icon: 'description' },
  { id: 'architecture', label: 'Architecture', icon: 'hub' },
  { id: 'deployments', label: 'Deployments', icon: 'rocket_launch' },
  { id: 'settings', label: 'Settings', icon: 'settings' },
];

export function ProjectDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const {
    project,
    rawProject,
    tasks,
    rawTasks,
    requirements,
    activeTab,
    setActiveTab,
    loading,
    error,
    refetch,
    sprints,
    selectedSprintId,
    setSelectedSprintId,
  } = useProjectDetail(id);

  const { members } = useProjectMembers(id ?? undefined);
  const currentUser = getCurrentUser();
  const canViewDeployments = useMemo(() => {
    if (isSystemAdmin(currentUser)) return true;
    if (!currentUser || !members.length) return false;
    const myMember = members.find((m) => m.id === currentUser.id);
    if (!myMember) return false;
    const roles = myMember.roles.map((r) => r.toLowerCase());
    return roles.includes('owner') || roles.includes('admin') || roles.includes('developer');
  }, [currentUser, members]);
  const canManageProject = useMemo(() => {
    if (isSystemAdmin(currentUser)) return true;
    if (!currentUser || !members.length) return false;
    const myMember = members.find((member) => member.id === currentUser.id);
    if (!myMember) return false;
    const roles = myMember.roles.map((role) => role.toLowerCase());
    return roles.includes('owner') || roles.includes('admin');
  }, [currentUser, members]);

  const visibleTabs = useMemo(
    () => (canViewDeployments ? tabs : tabs.filter((t) => t.id !== 'deployments')),
    [canViewDeployments]
  );

  useEffect(() => {
    if (activeTab === 'deployments' && !canViewDeployments) {
      setActiveTab('summary');
    }
  }, [activeTab, canViewDeployments, setActiveTab]);

  const [showCreateTaskModal, setShowCreateTaskModal] = useState(false);
  const [showRequirementForm, setShowRequirementForm] = useState(false);
  const [showRequirementDetail, setShowRequirementDetail] = useState(false);
  const [showRequirementBreakdown, setShowRequirementBreakdown] = useState(false);
  const [viewingRequirement, setViewingRequirement] = useState<typeof requirements[0] | null>(null);
  const [breakdownRequirement, setBreakdownRequirement] = useState<typeof requirements[0] | null>(null);
  const [editingRequirement, setEditingRequirement] = useState<typeof requirements[0] | null>(null);
  const [logsTask, setLogsTask] = useState<KanbanTask | null>(null);
  const [logsAttemptId, setLogsAttemptId] = useState<string | null>(null);
  const [showAssistant, setShowAssistant] = useState(false);
  const [showRepositoryAccessModal, setShowRepositoryAccessModal] = useState(false);
  const [recheckingRepositoryAccess, setRecheckingRepositoryAccess] = useState(false);
  const [repositoryFeedback, setRepositoryFeedback] = useState<string | null>(null);
  const [showLinkForkModal, setShowLinkForkModal] = useState(false);
  const [forkRepositoryUrl, setForkRepositoryUrl] = useState('');
  const [linkForkPending, setLinkForkPending] = useState(false);
  const [linkForkError, setLinkForkError] = useState<string | null>(null);
  const [autoForkPending, setAutoForkPending] = useState(false);

  const {
    session: assistantSession,
    messages: assistantMessages,
    loading: assistantLoading,
    error: assistantError,
    agentActive: assistantAgentActive,
    starting: assistantStarting,
    createSession: createAssistantSession,
    startAgent: startAssistantAgent,
    sendMessage: sendAssistantMessage,
    refreshSession: refreshAssistantSession,
    loadSession: loadAssistantSession,
    endSession: endAssistantSession,
  } = useProjectAssistant(id ?? undefined);

  const handleOpenAssistant = async () => {
    if (!id) return;
    // forceNew=false: tái sử dụng session đang active nếu có (tránh tạo quá nhiều session)
    await createAssistantSession(false);
    setShowAssistant(true);
  };

  const handleTaskClick = (taskId: string) => {
    // Navigate to task detail page
    navigate(`/projects/${id}/task/${taskId}`);
  };

  const handleViewLogs = async (taskId: string) => {
    // Find task and open logs modal
    const task = tasks.find(t => t.id === taskId);
    if (task) {
      setLogsTask(task);

      // Fetch attempts and find latest one
      try {
        const attempts = await getTaskAttempts(taskId);
        if (attempts.length > 0) {
          // Sort by created_at descending and get the first (latest) one
          const sortedAttempts = [...attempts].sort(
            (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
          );
          setLogsAttemptId(sortedAttempts[0].id);
        } else {
          setLogsAttemptId(null);
        }
      } catch (err) {
        logger.error('Failed to fetch attempts:', err);
        setLogsAttemptId(null);
      }
    }
  };

  if (loading) {
    return (
      <AppShell>
        <div className="flex-1 overflow-y-auto p-6 md:p-8">
          <div className="max-w-[1600px] mx-auto">
            <ProjectDetailSkeleton />
          </div>
        </div>
      </AppShell>
    );
  }

  if (error || !project) {
    return (
      <AppShell>
        <div className="flex-1 overflow-y-auto p-6 md:p-8">
          <div className="max-w-[1600px] mx-auto">
            <div className="bg-red-100 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg">
              {error || 'Project not found'}
            </div>
          </div>
        </div>
      </AppShell>
    );
  }

  const handleAddTask = () => {
    setShowCreateTaskModal(true);
  };

  const handleRecheckRepositoryAccess = async () => {
    if (!id) return;

    setRecheckingRepositoryAccess(true);
    setRepositoryFeedback(null);

    try {
      const response = await recheckProjectRepositoryAccess(id);
      setRepositoryFeedback(response.recommended_action || 'Repository access was refreshed.');
      refetch();
    } catch (err) {
      setRepositoryFeedback(err instanceof Error ? err.message : 'Failed to re-check repository access.');
    } finally {
      setRecheckingRepositoryAccess(false);
    }
  };

  const handleLinkExistingFork = async () => {
    if (!id || !forkRepositoryUrl.trim()) {
      setLinkForkError('Fork repository URL is required.');
      return;
    }

    setLinkForkPending(true);
    setLinkForkError(null);

    try {
      const response = await linkExistingFork(id, {
        repository_url: forkRepositoryUrl.trim(),
      });
      setRepositoryFeedback(
        response.recommended_action || 'Writable fork linked successfully.'
      );
      setShowLinkForkModal(false);
      setForkRepositoryUrl('');
      refetch();
    } catch (err) {
      setLinkForkError(
        err instanceof Error ? err.message : 'Failed to link writable fork.'
      );
    } finally {
      setLinkForkPending(false);
    }
  };

  const handleCreateProjectFork = async () => {
    if (!id) return;

    setAutoForkPending(true);
    setRepositoryFeedback(null);

    try {
      const response = await createProjectFork(id);
      setRepositoryFeedback(
        response.recommended_action || `Writable fork created: ${response.created_repository_url}`
      );
      refetch();
    } catch (err) {
      setRepositoryFeedback(
        err instanceof Error ? err.message : 'Failed to create writable fork.'
      );
    } finally {
      setAutoForkPending(false);
    }
  };

  const repositoryContext = normalizeRepositoryContext(rawProject?.repository_context);
  const repositorySummary = getRepositoryAccessSummary(repositoryContext);
  const repositoryTone = getRepositoryAccessTone(repositoryContext);
  const repositoryHref = getRepositoryHref(project.repositoryUrl);
  const repositoryStatusClass = repositoryTone === 'success'
    ? 'text-emerald-500 dark:text-emerald-400'
    : repositoryTone === 'warning'
      ? 'text-red-500 dark:text-red-400'
      : 'text-slate-400 dark:text-slate-500';
  const repositoryStatusIcon = repositoryTone === 'success'
    ? 'check_circle'
    : repositoryTone === 'warning'
      ? 'error'
      : 'help';
  const repositoryModalClass = repositoryTone === 'success'
    ? 'border-emerald-200 bg-emerald-50 dark:border-emerald-500/30 dark:bg-emerald-500/10'
    : repositoryTone === 'warning'
      ? 'border-red-200 bg-red-50 dark:border-red-500/30 dark:bg-red-500/10'
      : 'border-slate-200 bg-slate-50 dark:border-slate-500/30 dark:bg-slate-500/10';
  const repositoryBadgeClass = repositoryTone === 'success'
    ? 'bg-emerald-500/10 text-emerald-700 dark:text-emerald-300'
    : repositoryTone === 'warning'
      ? 'bg-red-500/10 text-red-700 dark:text-red-300'
      : 'bg-slate-500/10 text-slate-700 dark:text-slate-300';
  const repositoryStatusTitle = `${repositorySummary.title}. ${repositorySummary.description} ${repositorySummary.action}`;
  const shouldOfferForkLink =
    canManageProject &&
    (!repositoryContext.can_push || !repositoryContext.can_open_change_request);
  const shouldOfferAutoFork =
    shouldOfferForkLink &&
    Boolean(repositoryContext.can_fork);

  return (
    <AppShell>
      <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide">
        <div className="max-w-[1600px] mx-auto flex flex-col gap-6">
          {/* Header */}
          <div className="flex flex-col md:flex-row md:items-start justify-between gap-4">
            <div className="flex items-start gap-3">
              <button
                onClick={() => navigate('/projects')}
                className="mt-1 text-muted-foreground hover:text-primary transition-colors"
              >
                <span className="material-symbols-outlined">arrow_back</span>
              </button>
              <div>
                <div className="flex items-center gap-2 mb-1">
                  <h1 className="text-2xl md:text-3xl font-bold text-card-foreground">
                    {project.name}
                  </h1>
                  <button
                    type="button"
                    onClick={() => setShowRepositoryAccessModal(true)}
                    className="inline-flex items-center justify-center rounded-full transition-transform hover:scale-105 focus:outline-none focus:ring-2 focus:ring-primary/40"
                    title={repositoryStatusTitle}
                    aria-label={`Repository access status: ${getRepositoryModeLabel(repositoryContext.access_mode)}`}
                  >
                    <span className={`material-symbols-outlined text-[22px] ${repositoryStatusClass}`}>
                      {repositoryStatusIcon}
                    </span>
                  </button>
                </div>
                <div className="flex items-center gap-3 text-sm">
                  <a
                    href={repositoryHref}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-muted-foreground hover:text-primary transition-colors flex items-center gap-1"
                  >
                    <span className="material-symbols-outlined text-[16px]">link</span>
                    {project.repositoryUrl}
                  </a>
                  <span className="text-muted-foreground/50">|</span>
                  <span className="text-muted-foreground flex items-center gap-1">
                    <span className="material-symbols-outlined text-[16px]">commit</span>
                    {project.branch}
                  </span>
                </div>
                {repositoryFeedback && (
                  <p className="mt-2 text-xs text-muted-foreground">{repositoryFeedback}</p>
                )}
              </div>
            </div>
            <div className="flex items-center gap-3">
              <SprintSelector
                sprints={sprints}
                selectedSprintId={selectedSprintId}
                onSelectSprint={setSelectedSprintId}
              />
              <button
                onClick={() => setActiveTab('settings')}
                className={`px-4 py-2 border text-sm font-medium rounded-lg transition-colors flex items-center gap-2 ${activeTab === 'settings'
                    ? 'bg-primary/10 border-primary text-primary'
                    : 'bg-card border-border text-card-foreground hover:bg-muted'
                  }`}
              >
                <span className="material-symbols-outlined text-[18px]">settings</span>
                Settings
              </button>
              <button
                onClick={handleAddTask}
                className="px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all"
              >
                <span className="material-symbols-outlined text-[18px]">add</span>
                Add Task
              </button>
            </div>
          </div>

          {/* Stats Cards */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="p-4 bg-card rounded-xl border border-border">
              <div className="flex items-center gap-3 mb-2">
                <div className="p-2 rounded-lg bg-primary/10 text-primary">
                  <span className="material-symbols-outlined">smart_toy</span>
                </div>
                <span className="text-sm font-medium text-muted-foreground">Active Agents</span>
              </div>
              <p className="text-2xl font-bold text-card-foreground">{project.stats.activeAgents}</p>
            </div>
            <div className="p-4 bg-card rounded-xl border border-border">
              <div className="flex items-center gap-3 mb-2">
                <div className="p-2 rounded-lg bg-amber-500/10 dark:bg-amber-500/20 text-amber-500">
                  <span className="material-symbols-outlined">rate_review</span>
                </div>
                <span className="text-sm font-medium text-muted-foreground">Pending Review</span>
              </div>
              <p className="text-2xl font-bold text-card-foreground">{project.stats.pendingReview}</p>
            </div>
            <div className="p-4 bg-card rounded-xl border border-border">
              <div className="flex items-center gap-3 mb-2">
                <div className="p-2 rounded-lg bg-red-500/10 dark:bg-red-500/20 text-red-500">
                  <span className="material-symbols-outlined">bug_report</span>
                </div>
                <span className="text-sm font-medium text-muted-foreground">Critical Bugs</span>
              </div>
              <p className="text-2xl font-bold text-card-foreground">{project.stats.criticalBugs}</p>
            </div>
            <div className="p-4 bg-card rounded-xl border border-border">
              <div className="flex items-center gap-3 mb-2">
                <div className="p-2 rounded-lg bg-green-500/10 dark:bg-green-500/20 text-green-500">
                  <span className="material-symbols-outlined">check_circle</span>
                </div>
                <span className="text-sm font-medium text-muted-foreground">Build Status</span>
              </div>
              <p className="text-2xl font-bold text-green-600 dark:text-green-400">{project.stats.buildStatus}%</p>
            </div>
          </div>

          {/* Tabs */}
          <div className="border-b border-border">
            <nav className="flex gap-0 -mb-px overflow-x-auto no-scrollbar">
              {visibleTabs.map((tab) => (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className={`flex items-center gap-2 px-5 py-3 text-sm font-medium border-b-2 transition-colors whitespace-nowrap ${activeTab === tab.id
                    ? 'border-primary text-primary'
                    : 'border-transparent text-muted-foreground hover:text-card-foreground hover:border-border/80'
                    }`}
                >
                  <span className="material-symbols-outlined text-[18px]">{tab.icon}</span>
                  {tab.label}
                </button>
              ))}
            </nav>
          </div>

          {/* Tab Content */}
          <div className="flex-1">
            <ErrorBoundary fallback={
              <div className="p-8 mt-4 flex flex-col items-center justify-center bg-card rounded-xl border border-border">
                <span className="material-symbols-outlined text-4xl text-destructive mb-3">error</span>
                <h3 className="text-destructive font-bold text-lg mb-2">Tab failed to load</h3>
                <p className="text-muted-foreground text-sm text-center">An error occurred while rendering this tab's content. We've caught the error to prevent the app from crashing.</p>
                <button
                  onClick={() => setActiveTab('summary')}
                  className="mt-4 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg transition-colors text-sm font-medium"
                >
                  Return to Summary
                </button>
              </div>
            }>
              {activeTab === 'summary' && (
                <SummaryTab
                  projectId={project.id}
                  description={rawProject?.description}
                  repositoryUrl={project.repositoryUrl}
                  metadata={rawProject?.metadata}
                  rawProject={rawProject ?? undefined}
                  requirements={requirements}
                  sprints={sprints}
                  selectedSprintId={selectedSprintId}
                  onSelectSprint={setSelectedSprintId}
                  onNavigateTab={setActiveTab}
                  onRefreshProject={refetch}
                />
              )}
              {activeTab === 'kanban' && (
                <TaskListTab
                  tasks={tasks}
                  projectId={project.id}
                  sprints={sprints}
                  selectedSprintId={selectedSprintId}
                  onSelectSprint={setSelectedSprintId}
                  onRefreshProject={refetch}
                  onAddTask={handleAddTask}
                  onTaskClick={handleTaskClick}
                  onViewLogs={handleViewLogs}
                />
              )}
              {activeTab === 'requirements' && (
                <RequirementsTab
                  requirements={requirements}
                  rawTasks={rawTasks}
                  onAddRequirement={() => {
                    setEditingRequirement(null);
                    setShowRequirementForm(true);
                  }}
                  onRequirementClick={(req) => {
                    setViewingRequirement(req);
                    setShowRequirementDetail(true);
                  }}
                  onStatusChange={async (reqId, newStatus) => {
                    if (project) await updateRequirement(project.id, reqId, { status: newStatus });
                    refetch();
                  }}
                  onAnalyzeWithAI={() => {/* Phase 3 */}}
                  onImport={() => {/* Phase 2 placeholder */}}
                />
              )}
              {activeTab === 'architecture' && (
                <ArchitectureTab projectId={project.id} />
              )}
              {activeTab === 'deployments' && (
                <DeploymentsTab projectId={project.id} />
              )}
              {activeTab === 'settings' && (
                <SettingsTab
                  projectId={project.id}
                  projectName={project.name}
                  repositoryUrl={project.repositoryUrl}
                  requireReview={rawProject?.require_review ?? true}
                  onRefresh={refetch}
                />
              )}
            </ErrorBoundary>
          </div>
        </div>
      </div>

      <CreateTaskModal
        isOpen={showCreateTaskModal}
        onClose={() => setShowCreateTaskModal(false)}
        projectId={project.id}
        projectName={project.name}
        repositoryContext={rawProject?.repository_context}
        sprints={sprints}
        members={members}
      />

      <RequirementFormModal
        isOpen={showRequirementForm}
        onClose={() => {
          setShowRequirementForm(false);
          setEditingRequirement(null);
        }}
        projectId={project.id}
        requirement={editingRequirement}
        onSuccess={refetch}
      />

      <RequirementDetailModal
        isOpen={showRequirementDetail}
        onClose={() => {
          setShowRequirementDetail(false);
          setViewingRequirement(null);
        }}
        projectId={project.id}
        requirement={viewingRequirement}
        linkedTasks={viewingRequirement ? rawTasks.filter(t => t.requirement_id === viewingRequirement.id) : []}
        onEdit={() => {
          setEditingRequirement(viewingRequirement);
          setShowRequirementDetail(false);
          setShowRequirementForm(true);
        }}
        onRefresh={refetch}
        onBreakIntoTasks={() => {
          setBreakdownRequirement(viewingRequirement);
          setShowRequirementDetail(false);
          setShowRequirementBreakdown(true);
        }}
      />

      <RequirementBreakdownModal
        isOpen={showRequirementBreakdown}
        onClose={() => {
          setShowRequirementBreakdown(false);
          setBreakdownRequirement(null);
        }}
        projectId={project.id}
        requirement={breakdownRequirement}
        sprints={sprints}
        members={members}
        onCreated={refetch}
      />

      {showAssistant && project && (
        <ProjectAssistantPanel
          projectId={project.id}
          sessionId={assistantSession?.id ?? null}
          sessionStatus={assistantSession?.status}
          messages={assistantMessages}
          error={assistantError}
          agentActive={assistantAgentActive}
          starting={assistantStarting}
          onStartAgent={startAssistantAgent}
          onSendMessage={sendAssistantMessage}
          onRefreshMessages={refreshAssistantSession}
          onLoadSession={loadAssistantSession}
          onEndSession={endAssistantSession}
          onNewSession={() => createAssistantSession(true)}
          loading={assistantLoading}
          onClose={() => setShowAssistant(false)}
          onRefreshProject={refetch}
        />
      )}

      {project && !loading && !showAssistant && (
        <FloatingChatButton onClick={handleOpenAssistant} />
      )}

      {logsTask && (
        <ViewLogsModal
          isOpen={!!logsTask}
          onClose={() => {
            setLogsTask(null);
            setLogsAttemptId(null);
          }}
          task={logsTask}
          projectId={id}
          initialAttemptId={logsAttemptId}
        />
      )}

      {showRepositoryAccessModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
          <div
            className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
            onClick={() => {
              if (!recheckingRepositoryAccess && !autoForkPending) {
                setShowRepositoryAccessModal(false);
              }
            }}
          />
          <div className="relative w-full max-w-2xl bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
            <div className="px-6 py-5 border-b border-border bg-muted flex items-start justify-between gap-4">
              <div className="flex items-start gap-3">
                <span className={`material-symbols-outlined text-[24px] mt-0.5 ${repositoryStatusClass}`}>
                  {repositoryStatusIcon}
                </span>
                <div>
                  <h2 className="text-lg font-bold text-card-foreground">Repository access</h2>
                  <p className="text-sm text-muted-foreground mt-1">
                    {repositorySummary.title}
                  </p>
                </div>
              </div>
              <button
                onClick={() => {
                  if (!recheckingRepositoryAccess && !autoForkPending) {
                    setShowRepositoryAccessModal(false);
                  }
                }}
                className="text-muted-foreground hover:text-card-foreground transition-colors"
              >
                <span className="material-symbols-outlined">close</span>
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div className={`rounded-xl border p-4 ${repositoryModalClass}`}>
                <div className="flex items-center gap-2 flex-wrap">
                  <span className={`px-2.5 py-1 rounded-full text-[11px] font-semibold ${repositoryBadgeClass}`}>
                    {getRepositoryModeLabel(repositoryContext.access_mode)}
                  </span>
                  <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold border border-border bg-card/70 text-card-foreground">
                    {repositoryContext.can_clone ? 'Clone ready' : 'Clone unknown'}
                  </span>
                  <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold border border-border bg-card/70 text-card-foreground">
                    {repositoryContext.can_push ? 'Push enabled' : 'Push blocked'}
                  </span>
                  <span className="px-2.5 py-1 rounded-full text-[11px] font-semibold border border-border bg-card/70 text-card-foreground">
                    {repositoryContext.can_open_change_request ? 'PR/MR enabled' : 'PR/MR blocked'}
                  </span>
                </div>
                <p className="text-sm text-muted-foreground mt-3">{repositorySummary.description}</p>
                <p className="text-xs text-muted-foreground mt-2">{repositorySummary.action}</p>
              </div>

              {repositoryContext.upstream_repository_url && (
                <div className="rounded-lg border border-border bg-muted/50 p-4">
                  <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                    Upstream repository
                  </p>
                  <p className="text-sm text-card-foreground mt-2 break-all">
                    {repositoryContext.upstream_repository_url}
                  </p>
                </div>
              )}

              {repositoryFeedback && (
                <div className="rounded-lg border border-border bg-muted/50 px-4 py-3">
                  <p className="text-sm text-card-foreground">{repositoryFeedback}</p>
                </div>
              )}
            </div>

            <div className="px-6 py-4 border-t border-border bg-muted/50 flex flex-wrap justify-end gap-3">
              {canManageProject && (
                <button
                  onClick={() => {
                    void handleRecheckRepositoryAccess();
                  }}
                  disabled={recheckingRepositoryAccess}
                  className="px-4 py-2 rounded-lg text-sm font-semibold border border-border bg-card text-card-foreground hover:bg-muted transition-colors disabled:opacity-60 flex items-center gap-2"
                >
                  <span className={`material-symbols-outlined text-[16px] ${recheckingRepositoryAccess ? 'animate-spin' : ''}`}>
                    sync
                  </span>
                  {recheckingRepositoryAccess ? 'Re-checking...' : 'Re-check access'}
                </button>
              )}
              {shouldOfferAutoFork && (
                <button
                  onClick={() => {
                    void handleCreateProjectFork();
                  }}
                  disabled={autoForkPending}
                  className="px-4 py-2 rounded-lg text-sm font-semibold border border-border bg-card text-card-foreground hover:bg-muted transition-colors disabled:opacity-60 flex items-center gap-2"
                >
                  <span className={`material-symbols-outlined text-[16px] ${autoForkPending ? 'animate-spin' : ''}`}>
                    fork_right
                  </span>
                  {autoForkPending ? 'Creating fork...' : 'Create fork automatically'}
                </button>
              )}
              {shouldOfferForkLink && (
                <button
                  onClick={() => {
                    setShowRepositoryAccessModal(false);
                    setShowLinkForkModal(true);
                    setLinkForkError(null);
                    setForkRepositoryUrl(
                      repositoryContext.writable_repository_url ||
                        repositoryContext.upstream_repository_url ||
                        ''
                    );
                  }}
                  className="px-4 py-2 rounded-lg text-sm font-semibold border border-border bg-card text-card-foreground hover:bg-muted transition-colors flex items-center gap-2"
                >
                  <span className="material-symbols-outlined text-[16px]">fork_right</span>
                  Link writable fork
                </button>
              )}
            </div>
          </div>
        </div>
      )}

      {showLinkForkModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 font-display">
          <div
            className="absolute inset-0 bg-black/70 backdrop-blur-[2px]"
            onClick={() => {
              if (!linkForkPending) {
                setShowLinkForkModal(false);
                setLinkForkError(null);
              }
            }}
          />
          <div className="relative w-full max-w-lg bg-card border border-border rounded-2xl shadow-2xl overflow-hidden">
            <div className="px-6 py-5 border-b border-border bg-muted flex items-start justify-between gap-4">
              <div>
                <h2 className="text-lg font-bold text-card-foreground">Link Existing Fork</h2>
                <p className="text-sm text-muted-foreground mt-1">
                  Attach a writable GitHub or GitLab fork so ACPMS can push changes and open PRs or MRs.
                </p>
              </div>
              <button
                onClick={() => {
                  if (!linkForkPending) {
                    setShowLinkForkModal(false);
                    setLinkForkError(null);
                  }
                }}
                className="text-muted-foreground hover:text-card-foreground transition-colors"
              >
                <span className="material-symbols-outlined">close</span>
              </button>
            </div>

            <div className="p-6 space-y-4">
              <div className="rounded-lg border border-border bg-muted/50 p-4">
                <p className="text-xs font-semibold uppercase tracking-[0.16em] text-muted-foreground">
                  Upstream repository
                </p>
                <p className="text-sm text-card-foreground mt-2 break-all">
                  {repositoryContext.upstream_repository_url || project.repositoryUrl}
                </p>
              </div>

              <div>
                <label className="block text-sm font-bold text-card-foreground mb-1.5">
                  Writable fork URL
                </label>
                <input
                  type="text"
                  value={forkRepositoryUrl}
                  onChange={(event) => setForkRepositoryUrl(event.target.value)}
                  placeholder="https://github.com/your-user/repo-fork"
                  className="w-full bg-muted border border-border rounded-lg py-2.5 px-4 text-card-foreground focus:ring-primary focus:border-primary"
                />
              </div>

              {linkForkError && (
                <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-3 dark:border-red-500/30 dark:bg-red-500/15">
                  <p className="text-sm text-red-700 dark:text-red-200">{linkForkError}</p>
                </div>
              )}
            </div>

            <div className="px-6 py-4 border-t border-border bg-muted/50 flex justify-end gap-3">
              <button
                onClick={() => {
                  if (!linkForkPending) {
                    setShowLinkForkModal(false);
                    setLinkForkError(null);
                  }
                }}
                disabled={linkForkPending}
                className="px-4 py-2 text-sm font-medium text-muted-foreground hover:text-card-foreground transition-colors disabled:opacity-50"
              >
                Cancel
              </button>
              <button
                onClick={() => {
                  void handleLinkExistingFork();
                }}
                disabled={linkForkPending || !forkRepositoryUrl.trim()}
                className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 transition-all disabled:opacity-50 flex items-center gap-2"
              >
                {linkForkPending ? (
                  <>
                    <span className="inline-block w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    Linking...
                  </>
                ) : (
                  <>
                    <span className="material-symbols-outlined text-[18px]">fork_right</span>
                    Link Fork
                  </>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </AppShell>
  );
}
