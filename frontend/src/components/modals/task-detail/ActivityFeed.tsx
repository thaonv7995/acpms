interface ActivityFeedProps {
    activities: Array<{
        id: string;
        author: string;
        avatar?: string;
        timestamp: string;
        message: string;
    }>;
}

export function ActivityFeed({ activities }: ActivityFeedProps) {
    return (
        <div>
            <h3 className="text-xs font-bold text-slate-500 uppercase mb-3 flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px]">chat</span>
                Activity
            </h3>
            {activities.map((activity) => (
                <div key={activity.id} className="flex gap-4">
                    <div className="size-8 rounded-full bg-slate-200 dark:bg-slate-700 flex-shrink-0 flex items-center justify-center text-xs font-bold text-slate-600 dark:text-slate-300">
                        {activity.avatar || 'AI'}
                    </div>
                    <div className="flex-1">
                        <div className="bg-slate-50 dark:bg-[#161b22] border border-slate-200 dark:border-slate-700 rounded-lg p-3">
                            <p className="text-xs font-bold text-slate-900 dark:text-white mb-1">
                                {activity.author}{' '}
                                <span className="text-slate-400 font-normal ml-1">{activity.timestamp}</span>
                            </p>
                            <p className="text-sm text-slate-600 dark:text-slate-300">{activity.message}</p>
                        </div>
                    </div>
                </div>
            ))}
        </div>
    );
}
