// StatCard Component
import React from 'react';

interface StatCardProps {
    icon: string;
    iconBgColor: string;
    iconTextColor: string;
    label: string;
    value: React.ReactNode;
    badge?: {
        text: string;
        variant: 'success' | 'warning' | 'info' | 'danger';
    };
    progress?: {
        value: number;
        color: string;
    };
}

const badgeStyles = {
    success: 'text-green-600 bg-green-100 dark:bg-green-500/20 dark:text-green-400',
    warning: 'text-orange-600 bg-orange-100 dark:bg-orange-500/20 dark:text-orange-400',
    info: 'text-slate-600 bg-slate-100 dark:bg-slate-500/20 dark:text-slate-300',
    danger: 'text-red-600 bg-red-100 dark:bg-red-500/20 dark:text-red-400',
};

export function StatCard({
    icon,
    iconBgColor,
    iconTextColor,
    label,
    value,
    badge,
    progress,
}: StatCardProps) {
    return (
        <div className="p-5 rounded-xl bg-card border border-border shadow-sm">
            <div className="flex justify-between items-start mb-4">
                <div className={`p-2 rounded-lg ${iconBgColor} ${iconTextColor}`}>
                    <span className="material-symbols-outlined">{icon}</span>
                </div>
                {badge && (
                    <span className={`flex items-center gap-1 text-xs font-medium px-2 py-1 rounded ${badgeStyles[badge.variant]}`}>
                        {badge.variant === 'success' && badge.text.includes('Live') && (
                            <span className="size-2 rounded-full bg-green-500 dark:bg-green-400 animate-pulse"></span>
                        )}
                        {badge.text}
                    </span>
                )}
            </div>
            <p className="text-sm text-muted-foreground font-medium">{label}</p>
            <h3 className="text-3xl font-bold mt-1 text-card-foreground">{value}</h3>
            {progress && (
                <div className="w-full bg-muted dark:bg-muted/50 h-1.5 rounded-full mt-3 overflow-hidden">
                    <div
                        className={`h-1.5 rounded-full ${progress.color}`}
                        style={{ width: `${progress.value}%` }}
                    ></div>
                </div>
            )}
        </div>
    );
}
