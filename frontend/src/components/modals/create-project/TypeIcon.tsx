/**
 * TypeIcon - Icon components for each project type
 *
 * Provides consistent iconography for the 6 project types:
 * - web: Globe/language icon
 * - mobile: Smartphone icon
 * - desktop: Desktop/monitor icon
 * - extension: Puzzle extension icon
 * - api: API/network icon
 * - microservice: Hub/container icon
 */

import type { ProjectType } from '../../../api/templates';

interface TypeIconProps {
  type: ProjectType;
  size?: 'sm' | 'md' | 'lg' | 'xl';
  className?: string;
}

// Size mappings for Material Symbols
const sizeClasses = {
  sm: 'text-[20px]',
  md: 'text-[24px]',
  lg: 'text-[32px]',
  xl: 'text-[48px]',
};

// Icon mapping for each project type
const typeIcons: Record<ProjectType, string> = {
  web: 'language',
  mobile: 'smartphone',
  desktop: 'desktop_windows',
  extension: 'extension',
  api: 'api',
  microservice: 'hub',
};

// Color mapping for each project type (Tailwind classes)
const typeColors: Record<ProjectType, { text: string; bg: string }> = {
  web: {
    text: 'text-blue-500',
    bg: 'bg-blue-500/10',
  },
  mobile: {
    text: 'text-purple-500',
    bg: 'bg-purple-500/10',
  },
  desktop: {
    text: 'text-emerald-500',
    bg: 'bg-emerald-500/10',
  },
  extension: {
    text: 'text-orange-500',
    bg: 'bg-orange-500/10',
  },
  api: {
    text: 'text-cyan-500',
    bg: 'bg-cyan-500/10',
  },
  microservice: {
    text: 'text-rose-500',
    bg: 'bg-rose-500/10',
  },
};

/**
 * TypeIcon component - displays the appropriate icon for a project type
 */
export function TypeIcon({ type, size = 'md', className = '' }: TypeIconProps) {
  const icon = typeIcons[type];
  const colors = typeColors[type];

  return (
    <span
      className={`material-symbols-outlined ${sizeClasses[size]} ${colors.text} ${className}`}
    >
      {icon}
    </span>
  );
}

interface TypeIconBadgeProps {
  type: ProjectType;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

// Badge size mappings
const badgeSizes = {
  sm: 'size-8',
  md: 'size-12',
  lg: 'size-16',
};

const badgeIconSizes: Record<string, 'sm' | 'md' | 'lg' | 'xl'> = {
  sm: 'sm',
  md: 'md',
  lg: 'lg',
};

/**
 * TypeIconBadge - Icon with colored circular background
 */
export function TypeIconBadge({ type, size = 'md', className = '' }: TypeIconBadgeProps) {
  const colors = typeColors[type];

  return (
    <div
      className={`${badgeSizes[size]} rounded-full ${colors.bg} flex items-center justify-center ${className}`}
    >
      <TypeIcon type={type} size={badgeIconSizes[size]} />
    </div>
  );
}

/**
 * Get icon name for a project type
 */
export function getTypeIcon(type: ProjectType): string {
  return typeIcons[type];
}

/**
 * Get color classes for a project type
 */
export function getTypeColors(type: ProjectType): { text: string; bg: string } {
  return typeColors[type];
}

/**
 * All project type icons as a collection (useful for iteration)
 */
export const projectTypeIcons = Object.entries(typeIcons).map(([type, icon]) => ({
  type: type as ProjectType,
  icon,
  colors: typeColors[type as ProjectType],
}));

export default TypeIcon;
