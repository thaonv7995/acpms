interface TaskDescriptionCardProps {
    description?: string;
}

export function TaskDescriptionCard({ description }: TaskDescriptionCardProps) {
    return (
        <div className="bg-card border border-border rounded-xl p-6">
            <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-4 flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px] text-muted-foreground">description</span>
                Description
            </h3>
            <div className="prose prose-sm dark:prose-invert max-w-none text-card-foreground">
                {description ? (
                    <p className="text-sm whitespace-pre-wrap leading-relaxed">{description}</p>
                ) : (
                    <p className="text-sm italic text-muted-foreground">No description provided.</p>
                )}
            </div>
        </div>
    );
}
