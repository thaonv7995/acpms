// ProjectsPage - Complete with working search, filters, and actions
import { useState, useCallback, useRef, useEffect } from 'react';
import { createPortal } from 'react-dom';
import { AppShell } from '../components/layout/AppShell';
import { ProjectCard } from '../components/projects';
import { CreateProjectModal, EditProjectModal, ConfirmModal } from '../components/modals';
import { useProjects } from '../hooks/useProjects';
import { deleteProject, updateProject } from '../api/projects';
import type { ProjectListItem } from '../types/project';

// Filter Dropdown Component
function FilterDropdown({
  label,
  options,
  selected,
  onSelect
}: {
  label: string;
  options: { value: string; label: string }[];
  selected: string[];
  onSelect: (values: string[]) => void;
}) {
  const [isOpen, setIsOpen] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const toggleOption = (value: string) => {
    if (selected.includes(value)) {
      onSelect(selected.filter(v => v !== value));
    } else {
      onSelect([...selected, value]);
    }
  };

  const handleButtonClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    setIsOpen(!isOpen);
  };

  const handleOptionClick = (value: string, e: React.MouseEvent) => {
    e.stopPropagation();
    toggleOption(value);
  };

  const handleClearClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    onSelect([]);
    setIsOpen(false);
  };

  const handleBackdropClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsOpen(false);
  };

  // Close dropdown when clicking outside
  useEffect(() => {
    if (!isOpen) return;

    const handleClickOutside = (event: MouseEvent) => {
      if (
        buttonRef.current &&
        !buttonRef.current.contains(event.target as Node) &&
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  // Calculate dropdown position
  const [dropdownPosition, setDropdownPosition] = useState({ top: 0, left: 0 });

  useEffect(() => {
    if (isOpen && buttonRef.current) {
      const rect = buttonRef.current.getBoundingClientRect();
      setDropdownPosition({
        top: rect.bottom + 4,
        left: rect.left,
      });
    }
  }, [isOpen]);

  const dropdownContent = isOpen ? (
    <>
      <div className="fixed inset-0 z-[100]" onClick={handleBackdropClick} />
      <div
        ref={dropdownRef}
        className="fixed z-[101] w-48 bg-card border border-border rounded-lg shadow-lg flex flex-col max-h-[300px]"
        style={{
          top: `${dropdownPosition.top}px`,
          left: `${dropdownPosition.left}px`,
        }}
        onClick={(e) => e.stopPropagation()}
      >
        <div className="overflow-y-auto py-1">
          {options.map((option) => (
            <button
              key={option.value}
              type="button"
              onClick={(e) => handleOptionClick(option.value, e)}
              className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
            >
              <span className={`material-symbols-outlined text-[16px] shrink-0 ${selected.includes(option.value) ? 'text-primary' : 'text-transparent'}`}>
                check
              </span>
              <span className="truncate">{option.label}</span>
            </button>
          ))}
        </div>
        {selected.length > 0 && (
          <>
            <hr className="border-border" />
            <button
              type="button"
              onClick={handleClearClick}
              className="w-full px-4 py-2 text-left text-sm text-muted-foreground hover:bg-muted shrink-0"
            >
              Clear filter
            </button>
          </>
        )}
      </div>
    </>
  ) : null;

  return (
    <div className="relative">
      <button
        ref={buttonRef}
        type="button"
        onClick={handleButtonClick}
        className={`flex items-center gap-2 h-10 px-3 border rounded-lg text-sm font-medium transition-colors whitespace-nowrap ${selected.length > 0
            ? 'bg-card border-border text-card-foreground'
            : 'bg-card border-border text-card-foreground hover:bg-muted'
          }`}
      >
        <span>{label}</span>
        <span className={`material-symbols-outlined text-[16px] text-muted-foreground transition-transform ${isOpen ? 'rotate-180' : ''}`}>expand_more</span>
      </button>
      {typeof document !== 'undefined' && createPortal(dropdownContent, document.body)}
    </div>
  );
}

// Loading skeleton
function ProjectsSkeleton() {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
      {[1, 2, 3, 4, 5, 6].map((i) => (
        <div key={i} className="animate-pulse bg-card rounded-xl p-5 border border-border">
          <div className="flex items-center gap-3 mb-4">
            <div className="size-10 rounded-lg bg-muted"></div>
            <div className="flex-1">
              <div className="h-5 w-32 bg-muted rounded mb-2"></div>
              <div className="h-3 w-20 bg-muted rounded"></div>
            </div>
          </div>
          <div className="flex gap-2 mb-4">
            <div className="h-6 w-16 bg-muted rounded"></div>
            <div className="h-6 w-16 bg-muted rounded"></div>
          </div>
          <div className="h-1.5 w-full bg-muted rounded mb-6"></div>
          <div className="h-8 w-full bg-muted rounded"></div>
        </div>
      ))}
    </div>
  );
}

const statusOptions = [
  { value: 'agent_reviewing', label: 'Agent Reviewing' },
  { value: 'active_coding', label: 'Active Coding' },
  { value: 'deploying', label: 'Deploying' },
  { value: 'completed', label: 'Completed' },
  { value: 'paused', label: 'Paused' },
];

const techStackOptions = [
  // Frontend
  { value: 'React', label: 'React' },
  { value: 'Vue.js', label: 'Vue.js' },
  { value: 'Angular', label: 'Angular' },
  { value: 'Next.js', label: 'Next.js' },
  { value: 'Svelte', label: 'Svelte' },
  { value: 'TypeScript', label: 'TypeScript' },
  { value: 'Tailwind CSS', label: 'Tailwind CSS' },
  // Backend
  { value: 'Node.js', label: 'Node.js' },
  { value: 'Python', label: 'Python' },
  { value: 'Rust', label: 'Rust' },
  { value: 'Go', label: 'Go' },
  { value: 'Java', label: 'Java' },
  { value: 'Express', label: 'Express' },
  { value: 'FastAPI', label: 'FastAPI' },
  { value: 'Django', label: 'Django' },
  { value: 'Spring Boot', label: 'Spring Boot' },
  // Database
  { value: 'PostgreSQL', label: 'PostgreSQL' },
  { value: 'MySQL', label: 'MySQL' },
  { value: 'MongoDB', label: 'MongoDB' },
  { value: 'Redis', label: 'Redis' },
  { value: 'SQLite', label: 'SQLite' },
  // Cloud & Infrastructure
  { value: 'AWS', label: 'AWS' },
  { value: 'Docker', label: 'Docker' },
  { value: 'Kubernetes', label: 'Kubernetes' },
  { value: 'Terraform', label: 'Terraform' },
  { value: 'GitLab CI', label: 'GitLab CI' },
  { value: 'GitHub Actions', label: 'GitHub Actions' },
  // Authentication & Security
  { value: 'OAuth', label: 'OAuth' },
  { value: 'JWT', label: 'JWT' },
  { value: 'Auth0', label: 'Auth0' },
  // Other
  { value: 'GraphQL', label: 'GraphQL' },
  { value: 'REST API', label: 'REST API' },
  { value: 'WebSocket', label: 'WebSocket' },
];

export function ProjectsPage() {
  const {
    filteredProjects,
    loading,
    error,
    searchQuery,
    setSearchQuery,
    filters,
    setFilters,
    refetch,
    page,
    setPage,
    totalPages,
    totalCount,
  } = useProjects();

  // React Query mutations
  // Modal states
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showEditModal, setShowEditModal] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [selectedProject, setSelectedProject] = useState<ProjectListItem | null>(null);
  const [actionMessage, setActionMessage] = useState('');
  const [isDeletingProject, setIsDeletingProject] = useState(false);

  const handleEdit = useCallback((projectId: string) => {
    const project = filteredProjects.find(p => p.id === projectId);
    if (project) {
      setSelectedProject(project);
      setShowEditModal(true);
    }
  }, [filteredProjects]);

  const handleSaveEdit = async (projectId: string, data: { name: string; description: string }) => {
    try {
      await updateProject(projectId, {
        name: data.name,
        description: data.description || undefined,
      });
      refetch();
      setActionMessage(`Project "${data.name}" updated successfully!`);
      setTimeout(() => setActionMessage(''), 3000);
      setShowEditModal(false);
      setSelectedProject(null);
    } catch (err) {
      throw err instanceof Error ? err : new Error('Failed to update project');
    }
  };

  const handleDelete = useCallback((projectId: string) => {
    const project = filteredProjects.find(p => p.id === projectId);
    if (project) {
      setSelectedProject(project);
      setShowDeleteConfirm(true);
    }
  }, [filteredProjects]);

  const handleConfirmDelete = async () => {
    if (!selectedProject) return;

    setIsDeletingProject(true);
    try {
      await deleteProject(selectedProject.id);
      refetch();
      setActionMessage(`Project "${selectedProject.name}" deleted!`);
      setTimeout(() => setActionMessage(''), 3000);
      setShowDeleteConfirm(false);
      setSelectedProject(null);
    } catch (err) {
      setActionMessage(err instanceof Error ? err.message : 'Failed to delete project');
      setTimeout(() => setActionMessage(''), 3000);
    } finally {
      setIsDeletingProject(false);
    }
  };

  // Pagination UI Helper
  const renderPagination = () => {
    if (totalPages <= 1) return null;

    const pages = [];
    const maxVisible = 5;
    let startPage = Math.max(1, page - Math.floor(maxVisible / 2));
    let endPage = startPage + maxVisible - 1;

    if (endPage > totalPages) {
      endPage = totalPages;
      startPage = Math.max(1, endPage - maxVisible + 1);
    }

    for (let i = startPage; i <= endPage; i++) {
      pages.push(
        <button
          key={i}
          onClick={() => setPage(i)}
          className={`h-10 w-10 rounded-lg flex items-center justify-center font-medium transition-colors ${page === i
              ? 'bg-primary text-primary-foreground'
              : 'bg-card text-muted-foreground hover:bg-muted border border-border shrink-0'
            }`}
        >
          {i}
        </button>
      );
    }

    return (
      <div className="flex flex-col sm:flex-row items-center justify-between gap-4 mt-8 pt-6 border-t border-border">
        <p className="text-sm text-muted-foreground whitespace-nowrap">
          Showing <strong>{(page - 1) * 9 + 1}</strong> to{' '}
          <strong>{Math.min(page * 9, totalCount)}</strong> of{' '}
          <strong>{totalCount}</strong> results
        </p>
        <div className="flex items-center gap-2 overflow-x-auto pb-2 sm:pb-0 w-full sm:w-auto">
          <button
            onClick={() => setPage(page - 1)}
            disabled={page === 1}
            className="h-10 px-3 rounded-lg flex items-center justify-center font-medium transition-colors bg-card text-muted-foreground hover:bg-muted border border-border disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
          >
            <span className="material-symbols-outlined text-[18px]">chevron_left</span>
          </button>

          {startPage > 1 && (
            <>
              <button
                onClick={() => setPage(1)}
                className="h-10 w-10 rounded-lg flex items-center justify-center font-medium transition-colors bg-card text-muted-foreground hover:bg-muted border border-border shrink-0"
              >
                1
              </button>
              {startPage > 2 && <span className="text-muted-foreground px-2">...</span>}
            </>
          )}

          {pages}

          {endPage < totalPages && (
            <>
              {endPage < totalPages - 1 && <span className="text-muted-foreground px-2">...</span>}
              <button
                onClick={() => setPage(totalPages)}
                className="h-10 w-10 rounded-lg flex items-center justify-center font-medium transition-colors bg-card text-muted-foreground hover:bg-muted border border-border shrink-0"
              >
                {totalPages}
              </button>
            </>
          )}

          <button
            onClick={() => setPage(page + 1)}
            disabled={page === totalPages}
            className="h-10 px-3 rounded-lg flex items-center justify-center font-medium transition-colors bg-card text-muted-foreground hover:bg-muted border border-border disabled:opacity-50 disabled:cursor-not-allowed shrink-0"
          >
            <span className="material-symbols-outlined text-[18px]">chevron_right</span>
          </button>
        </div>
      </div>
    );
  };

  return (
    <AppShell>
      <div className="flex-1 overflow-y-auto p-6 md:p-8 scrollbar-hide">
        <div className="max-w-[1600px] mx-auto flex flex-col gap-6">
          {/* Success Message */}
          {actionMessage && (
            <div className="fixed top-4 right-4 z-50 bg-green-100 dark:bg-green-500/20 border border-green-200 dark:border-green-500/30 text-green-700 dark:text-green-300 px-4 py-3 rounded-lg shadow-lg flex items-center gap-2 animate-fade-in">
              <span className="material-symbols-outlined text-green-500 dark:text-green-400">check_circle</span>
              {actionMessage}
            </div>
          )}

          {/* Header */}
          <div className="flex flex-col md:flex-row md:items-end justify-between gap-6 mb-8">
            <div className="flex flex-col gap-2">
              <h1 className="text-3xl md:text-4xl font-black tracking-tight text-card-foreground">Projects</h1>
              <p className="text-muted-foreground text-base md:text-lg">
                Manage your agentic workflows and coding tasks.
              </p>
            </div>
            <button
              onClick={() => setShowCreateModal(true)}
              className="flex items-center gap-2 bg-primary hover:bg-primary/90 text-primary-foreground font-bold py-2 px-4 rounded-lg transition-all shadow-sm shrink-0 h-10"
            >
              <span className="material-symbols-outlined text-[18px]">add</span>
              <span>Create New Project</span>
            </button>
          </div>

          {/* Filters */}
          <div className="flex flex-col lg:flex-row gap-3 mb-8 items-stretch lg:items-center">
            <div className="flex-1">
              <div className="relative group h-10">
                <div className="absolute inset-y-0 left-0 pl-3 flex items-center pointer-events-none text-muted-foreground">
                  <span className="material-symbols-outlined text-[20px]">search</span>
                </div>
                <input
                  type="text"
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="block w-full h-full pl-10 pr-10 rounded-lg border border-border bg-card text-card-foreground placeholder-muted-foreground focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-all text-sm"
                  placeholder="Search projects by name, ID, or description..."
                />
                {searchQuery && (
                  <button
                    onClick={() => setSearchQuery('')}
                    className="absolute inset-y-0 right-0 pr-3 flex items-center text-muted-foreground hover:text-card-foreground transition-colors"
                  >
                    <span className="material-symbols-outlined text-[18px]">close</span>
                  </button>
                )}
              </div>
            </div>
            <div className="flex items-center gap-2 overflow-x-auto pb-2 lg:pb-0 no-scrollbar">
              <FilterDropdown
                label="Status"
                options={statusOptions}
                selected={filters.status}
                onSelect={(values) => setFilters({ ...filters, status: values })}
              />
              <FilterDropdown
                label="Tech Stack"
                options={techStackOptions}
                selected={filters.techStack}
                onSelect={(values) => setFilters({ ...filters, techStack: values })}
              />
              {(filters.status.length > 0 || filters.techStack.length > 0) && (
                <button
                  onClick={() => setFilters({ status: [], techStack: [], hasAgent: null })}
                  className="h-10 px-3 text-sm text-muted-foreground hover:text-card-foreground transition-colors whitespace-nowrap flex items-center"
                >
                  Clear all
                </button>
              )}
            </div>
          </div>

          {/* Error */}
          {error && (
            <div className="bg-red-100 dark:bg-red-500/20 border border-red-200 dark:border-red-500/30 text-red-700 dark:text-red-400 px-4 py-3 rounded-lg">
              {error}
            </div>
          )}

          {/* Loading */}
          {loading ? (
            <ProjectsSkeleton />
          ) : (
            <>
              {/* Project Grid */}
              <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
                {filteredProjects.map((project) => (
                  <ProjectCard
                    key={project.id}
                    project={project}
                    onEdit={handleEdit}
                    onDelete={handleDelete}
                  />
                ))}
              </div>

              {/* Pagination */}
              {renderPagination()}

              {/* Empty State */}
              {filteredProjects.length === 0 && (
                <div className="text-center py-12">
                  <span className="material-symbols-outlined text-6xl text-muted-foreground/50 mb-4">folder_open</span>
                  <p className="text-muted-foreground mb-4">
                    {searchQuery || filters.status.length > 0 || filters.techStack.length > 0
                      ? 'No projects match your filters'
                      : 'No projects found'}
                  </p>
                  {(searchQuery || filters.status.length > 0 || filters.techStack.length > 0) ? (
                    <button
                      onClick={() => {
                        setSearchQuery('');
                        setFilters({ status: [], techStack: [], hasAgent: null });
                      }}
                      className="text-primary hover:underline"
                    >
                      Clear filters
                    </button>
                  ) : (
                    <button
                      onClick={() => setShowCreateModal(true)}
                      className="text-primary hover:underline"
                    >
                      Create your first project
                    </button>
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {/* Modals */}
      <CreateProjectModal
        isOpen={showCreateModal}
        onClose={() => setShowCreateModal(false)}
      />

      <EditProjectModal
        isOpen={showEditModal}
        onClose={() => { setShowEditModal(false); setSelectedProject(null); }}
        project={selectedProject}
        onSave={handleSaveEdit}
      />

      <ConfirmModal
        isOpen={showDeleteConfirm}
        onClose={() => { setShowDeleteConfirm(false); setSelectedProject(null); }}
        onConfirm={handleConfirmDelete}
        isLoading={isDeletingProject}
        title="Delete Project"
        message={`Are you sure you want to delete "${selectedProject?.name}"? This action cannot be undone.`}
        confirmText="Delete Project"
        confirmVariant="danger"
      />
    </AppShell>
  );
}
