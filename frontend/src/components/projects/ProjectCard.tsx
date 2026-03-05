// ProjectCard Component - with dropdown menu
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import type { ProjectListItem } from '../../types/project';

interface ProjectCardProps {
  project: ProjectListItem;
  onEdit?: (projectId: string) => void;
  onSettings?: (projectId: string) => void;
}

// Icon colors available for future use when project icons are displayed
// const iconColorClasses: Record<ProjectListItem['iconColor'], { bg: string; text: string }> = {
//   orange: { bg: 'bg-orange-500/10', text: 'text-orange-500' },
//   blue: { bg: 'bg-blue-500/10', text: 'text-blue-500' },
//   emerald: { bg: 'bg-emerald-500/10', text: 'text-emerald-500' },
//   purple: { bg: 'bg-purple-500/10', text: 'text-purple-500' },
//   primary: { bg: 'bg-primary/10', text: 'text-primary' },
// };

const statusColorClasses: Record<ProjectListItem['statusColor'], { dot: string; text: string; progress: string }> = {
  yellow: { dot: 'bg-yellow-500 animate-pulse', text: 'text-yellow-500', progress: 'bg-primary' },
  blue: { dot: 'bg-blue-400', text: 'text-blue-400', progress: 'bg-primary' },
  emerald: { dot: 'bg-emerald-400', text: 'text-emerald-400', progress: 'bg-emerald-500' },
  green: { dot: 'bg-green-400', text: 'text-green-400', progress: 'bg-green-500' },
  slate: { dot: 'bg-slate-400', text: 'text-slate-400', progress: 'bg-slate-500' },
};

// Material Symbols icon mapping for tech stacks (lowercase key)
const TECH_STACK_ICONS: Record<string, string> = {
  react: 'code',
  vite: 'bolt',
  'react + vite': 'code',
  vue: 'view_agenda',
  'vue.js': 'view_agenda',
  angular: 'view_module',
  next: 'arrow_forward',
  'next.js': 'arrow_forward',
  svelte: 'widgets',
  sveltekit: 'widgets',
  typescript: 'javascript',
  'tailwind css': 'format_paint',
  node: 'terminal',
  'node.js': 'terminal',
  python: 'code',
  rust: 'memory',
  go: 'terminal',
  java: 'coffee',
  express: 'api',
  fastapi: 'api',
  django: 'storage',
  'spring boot': 'eco',
  postgresql: 'database',
  mysql: 'database',
  mongodb: 'database',
  redis: 'storage',
  sqlite: 'database',
  docker: 'account_tree',
  kubernetes: 'hub',
  aws: 'cloud',
  terraform: 'architecture',
  graphql: 'hub',
  'rest api': 'api',
  websocket: 'wifi',
  tauri: 'desktop_windows',
  electron: 'computer',
  plasmo: 'extension',
};

function getTechStackIcon(tech: string): string {
  const key = tech.trim().toLowerCase();
  return TECH_STACK_ICONS[key] ?? 'code';
}

export function ProjectCard({ project, onEdit, onSettings }: ProjectCardProps) {
  const navigate = useNavigate();
  const [showMenu, setShowMenu] = useState(false);
  const statusColors = statusColorClasses[project.statusColor];

  const handleClick = () => {
    navigate(`/projects/${project.id}`);
  };

  const handleMenuClick = (e: React.MouseEvent) => {
    e.stopPropagation();
    setShowMenu(!showMenu);
  };

  const handleMenuAction = (action: 'edit' | 'settings', e: React.MouseEvent) => {
    e.stopPropagation();
    setShowMenu(false);

    switch (action) {
      case 'edit':
        onEdit?.(project.id);
        break;
      case 'settings':
        onSettings?.(project.id);
        navigate(`/projects/${project.id}?tab=settings`);
        break;
    }
  };

  return (
    <article
      onClick={handleClick}
      className="flex flex-col bg-card rounded-xl p-5 border border-border hover:border-border/80 transition-all shadow-sm hover:shadow-md group cursor-pointer relative"
    >
      {/* Header */}
      <div className="flex justify-between items-start mb-3">
        <div className="flex items-center gap-3">
          <span className="material-symbols-outlined text-[24px] text-muted-foreground">folder</span>
          <div>
            <h3 className="font-bold text-base leading-tight text-card-foreground">
              {project.name}
            </h3>
            <p className="text-xs text-muted-foreground mt-0.5">ID: #{project.id.slice(0, 8)}</p>
          </div>
        </div>

        {/* Menu Button */}
        <div className="relative">
          <button
            onClick={handleMenuClick}
            className="p-1 rounded-lg text-muted-foreground hover:text-card-foreground hover:bg-muted transition-colors"
          >
            <span className="material-symbols-outlined text-[20px]">more_vert</span>
          </button>

          {/* Dropdown Menu */}
          {showMenu && (
            <>
              <div
                className="fixed inset-0 z-10"
                onClick={(e) => { e.stopPropagation(); setShowMenu(false); }}
              />
              <div className="absolute right-0 top-full mt-1 z-20 w-40 bg-card border border-border rounded-lg shadow-lg py-1">
                <button
                  onClick={(e) => handleMenuAction('edit', e)}
                  className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
                >
                  <span className="material-symbols-outlined text-[18px]">edit</span>
                  Edit
                </button>
                <button
                  onClick={(e) => handleMenuAction('settings', e)}
                  className="w-full px-4 py-2 text-left text-sm text-card-foreground hover:bg-muted flex items-center gap-2"
                >
                  <span className="material-symbols-outlined text-[18px]">settings</span>
                  Settings
                </button>
              </div>
            </>
          )}
        </div>
      </div>

      {/* Description */}
      {project.description && (
        <p className="text-sm text-muted-foreground mb-3 line-clamp-2">
          {project.description}
        </p>
      )}

      {/* Tech Stack Tags */}
      <div className="mb-3 flex flex-wrap gap-1.5">
        {project.techStack.map((tech) => (
          <span
            key={tech}
            className="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-medium text-card-foreground bg-muted"
          >
            <span className="material-symbols-outlined text-[14px] text-muted-foreground">
              {getTechStackIcon(tech)}
            </span>
            {tech}
          </span>
        ))}
      </div>

      {/* Status & Progress */}
      <div className="flex items-center justify-between mb-2">
        <span className={`text-xs font-medium ${statusColors.text} flex items-center gap-1.5`}>
          <span className={`size-2 rounded-full ${statusColors.dot}`}></span>
          {project.statusLabel}
        </span>
        <span className="text-xs font-medium text-muted-foreground">{project.progress}%</span>
      </div>
      <div className="w-full bg-muted dark:bg-muted/50 rounded-full h-1.5 mb-4 overflow-hidden">
        <div className={`${statusColors.progress} h-1.5 rounded-full`} style={{ width: `${project.progress}%` }}></div>
      </div>

      {/* Footer */}
      <div className="mt-auto flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="material-symbols-outlined text-[18px] text-muted-foreground">smart_toy</span>
        </div>
        <span className="text-xs text-muted-foreground">{project.lastActivity}</span>
      </div>
    </article>
  );
}
