import type { ProjectListItem } from '../../../types/project';
import { Link } from 'react-router-dom';

interface ProjectSelectorProps {
    projects: ProjectListItem[];
    selectedProject: string;
    onProjectChange: (projectId: string) => void;
    loading?: boolean;
}

export function ProjectSelector({ projects, selectedProject, onProjectChange, loading }: ProjectSelectorProps) {
    return (
        <div>
            <label className="block text-sm font-bold text-card-foreground mb-1.5">
                Project <span className="text-red-500">*</span>
            </label>
            {projects.length === 0 && !loading ? (
                <div className="p-3 bg-yellow-50 dark:bg-yellow-500/20 border border-yellow-200 dark:border-yellow-500/30 rounded-lg text-sm text-yellow-700 dark:text-yellow-400">
                    No projects found. Please <Link to="/projects" className="underline font-medium hover:text-yellow-800 dark:hover:text-yellow-300">create a project</Link> from the Projects page first.
                </div>
            ) : (
                <div className="relative">
                    <select
                        value={selectedProject}
                        onChange={(e) => onProjectChange(e.target.value)}
                        disabled={loading || projects.length === 0}
                        className="w-full bg-muted border border-border rounded-lg px-3 py-2.5 text-sm text-card-foreground focus:ring-primary focus:border-primary appearance-none disabled:opacity-50"
                    >
                        <option value="">{loading ? 'Loading projects...' : '-- Select a project --'}</option>
                        {projects.map((project) => (
                            <option key={project.id} value={project.id}>
                                {project.name}
                            </option>
                        ))}
                    </select>
                    <span className="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground pointer-events-none material-symbols-outlined text-[18px]">expand_more</span>
                </div>
            )}
        </div>
    );
}
