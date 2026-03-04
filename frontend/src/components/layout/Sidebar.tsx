import { Link, useLocation, useNavigate } from 'react-router-dom';
import { useMemo } from 'react';
import { logout, getCurrentUser, isSystemAdmin } from '../../api/auth';
import { ThemeToggle } from '../ui/ThemeToggle';
import { useProjects } from '../../hooks/useProjects';

interface NavItem {
    path: string;
    label: string;
    icon: string;
}

interface NavButtonProps {
    item: NavItem;
    isActive: boolean;
}

function NavButton({ item, isActive }: NavButtonProps) {
    return (
        <Link
            to={item.path}
            className={`flex items-center gap-3 px-3 py-2.5 rounded-lg transition-colors w-full text-left group ${isActive
                ? 'bg-primary text-primary-foreground shadow-lg shadow-primary/25'
                : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                }`}
        >
            <span className={`material-symbols-outlined text-[20px] ${isActive ? 'material-symbols-filled' : ''}`}>
                {item.icon}
            </span>
            <span className={`hidden lg:block text-sm font-medium ${isActive ? 'font-bold' : ''}`}>
                {item.label}
            </span>
        </Link>
    );
}

export function Sidebar() {
    const location = useLocation();
    const navigate = useNavigate();
    const currentUser = useMemo(() => getCurrentUser(), []);
    const canViewAdministration = isSystemAdmin(currentUser);
    // Projects hook available for future use (e.g., project-specific navigation)
    useProjects();

    const userInitials = currentUser?.name
        ? currentUser.name.split(' ').map(n => n[0]).join('').toUpperCase().substring(0, 2)
        : 'U';

    // Tasks link always goes to /tasks (shows all projects by default)
    const tasksPath = '/tasks';

    const mainMenu: NavItem[] = [
        { path: '/dashboard', label: 'Dashboard', icon: 'grid_view' },
        { path: '/projects', label: 'Projects', icon: 'bar_chart' },
        { path: tasksPath, label: 'Tasks', icon: 'check_box' },
        // MR page hidden - use Kanban Diffs tab instead (has full diff viewing + PR creation)
        // { path: '/merge-requests', label: 'Merge Requests', icon: 'call_split' },
        { path: '/agent-logs', label: 'Agent Logs', icon: 'terminal' },
    ];

    const adminMenu: NavItem[] = [
        { path: '/users', label: 'User Management', icon: 'admin_panel_settings' },
        { path: '/settings', label: 'Settings', icon: 'settings' },
    ];

    const handleLogout = () => {
        logout();
        navigate('/login');
    };

    return (
        <aside className="w-20 lg:w-64 flex-shrink-0 flex flex-col border-r border-border bg-card transition-all duration-300 z-30">
            {/* Logo */}
            <div className="flex items-center gap-3 px-6 py-6 mb-2">
                <div className="size-8 rounded bg-primary flex items-center justify-center text-white shrink-0 shadow-lg shadow-primary/25 p-1.5">
                    <img src="/logo-symbol-white.svg" alt="ACPMS Logo" className="w-full h-full" />
                </div>
                <h1 className="hidden lg:block text-xl font-bold tracking-tight text-foreground">ACPMS</h1>
            </div>

            {/* Navigation */}
            <nav className="flex-1 flex flex-col gap-6 px-3 py-2 overflow-y-auto">
                {/* Main Menu Section */}
                <div className="flex flex-col gap-1">
                    <h3 className="hidden lg:block px-3 text-[11px] font-bold text-muted-foreground uppercase tracking-wider mb-2">Main Menu</h3>
                    {mainMenu.map((item) => {
                        // Special handling for Tasks link (can be /tasks or /projects/:id/tasks)
                        const isTasksLink = item.label === 'Tasks';
                        const isProjectsLink = item.label === 'Projects';
                        
                        let isActive = false;
                        if (isTasksLink) {
                            // Tasks is active if path includes /tasks
                            isActive = location.pathname.includes('/tasks');
                        } else if (isProjectsLink) {
                            // Projects is active if path starts with /projects but NOT /tasks
                            isActive = location.pathname.startsWith('/projects') && !location.pathname.includes('/tasks');
                        } else {
                            // Other items: exact match or starts with path + '/'
                            isActive = location.pathname === item.path || location.pathname.startsWith(item.path + '/');
                        }

                        return (
                            <NavButton
                                key={item.path}
                                item={item}
                                isActive={isActive}
                            />
                        );
                    })}
                </div>

                {/* Administration Section */}
                {canViewAdministration && (
                    <div className="flex flex-col gap-1">
                        <h3 className="hidden lg:block px-3 text-[11px] font-bold text-muted-foreground uppercase tracking-wider mb-2">Administration</h3>
                        {adminMenu.map((item) => (
                            <NavButton
                                key={item.path}
                                item={item}
                                isActive={location.pathname === item.path}
                            />
                        ))}
                    </div>
                )}
            </nav>

            {/* User Profile / Logout */}
            <div className="p-4 border-t border-border space-y-2">
                {/* Theme Toggle */}
                <div className="flex justify-center lg:justify-start px-2 py-2">
                    <ThemeToggle />
                </div>

                {/* Profile Link */}
                <Link
                    to="/profile"
                    className={`flex items-center gap-3 px-2 py-2 rounded-lg transition-colors w-full text-left ${location.pathname === '/profile'
                            ? 'bg-primary text-primary-foreground shadow-lg shadow-primary/25'
                            : 'text-muted-foreground hover:bg-accent hover:text-accent-foreground'
                        }`}
                >
                    <div className="relative shrink-0">
                        <div className={`size-9 rounded-full overflow-hidden border-2 flex items-center justify-center text-sm font-bold ${location.pathname === '/profile'
                                ? 'bg-white/20 border-white/30 text-white'
                                : 'bg-gradient-to-br from-blue-500 to-purple-500 border-slate-100 dark:border-slate-600 text-white'
                            }`}>
                            {currentUser?.avatar_url ? (
                                <img 
                                    src={currentUser.avatar_url} 
                                    alt={currentUser.name} 
                                    className="size-full object-cover"
                                    onError={(e) => {
                                        // Fallback to initials if image fails to load
                                        const target = e.target as HTMLImageElement;
                                        target.style.display = 'none';
                                        const parent = target.parentElement;
                                        if (parent && !parent.textContent) {
                                            parent.textContent = userInitials;
                                        }
                                    }}
                                />
                            ) : (
                                userInitials
                            )}
                        </div>
                        <span className="absolute bottom-0 right-0 size-2.5 bg-green-500 border-2 border-white dark:border-surface-dark rounded-full"></span>
                    </div>
                    <div className="hidden lg:flex flex-col overflow-hidden">
                        <span className={`text-sm font-bold truncate ${location.pathname === '/profile' ? 'text-primary-foreground' : 'text-foreground'}`}>
                            {currentUser?.name || 'User'}
                        </span>
                        <span className={`text-[11px] truncate ${location.pathname === '/profile' ? 'text-primary-foreground/70' : 'text-muted-foreground'}`}>
                            My Profile
                        </span>
                    </div>
                </Link>

                {/* Logout Button */}
                <button
                    onClick={handleLogout}
                    className="flex items-center gap-3 px-2 py-2 rounded-lg text-muted-foreground hover:bg-destructive/10 hover:text-destructive transition-colors w-full text-left"
                >
                    <span className="material-symbols-outlined text-[20px]">logout</span>
                    <span className="hidden lg:block text-sm font-medium">Logout</span>
                </button>

                {/* Attribution - full-width separator, link to LinkedIn */}
                <div className="mt-4 -mx-4 border-t border-border pt-3 px-4">
                    <p className="text-[10px] text-muted-foreground/70 text-center">
                        Developed by{' '}
                        <a
                            href="https://www.linkedin.com/in/thaonv795/"
                            target="_blank"
                            rel="noopener noreferrer"
                            className="no-underline hover:text-primary transition-colors"
                        >
                            @thaonv795
                        </a>
                    </p>
                </div>
            </div>
        </aside>
    );
}
