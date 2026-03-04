export function TaskActivityFeed() {
    return (
        <div className="bg-card border border-border rounded-xl p-6">
            <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-4 flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px] text-muted-foreground">forum</span>
                Activity
            </h3>
            <div className="space-y-4">
                <div className="flex gap-3">
                    <div className="size-8 rounded-full bg-primary/10 flex items-center justify-center shrink-0">
                        <span className="material-symbols-outlined text-primary text-[16px]">smart_toy</span>
                    </div>
                    <div className="flex-1">
                        <div className="flex items-center gap-2 mb-1">
                            <span className="text-sm font-medium text-card-foreground">Agent</span>
                            <span className="text-xs text-muted-foreground">just now</span>
                        </div>
                        <p className="text-sm text-muted-foreground leading-relaxed">
                            Ready to start working on this task. Click "Start Agent" to begin.
                        </p>
                    </div>
                </div>
            </div>
        </div>
    );
}
