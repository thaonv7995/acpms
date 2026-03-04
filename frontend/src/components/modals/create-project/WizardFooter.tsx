type Mode = 'initial' | 'import_gitlab' | 'new_type_select' | 'new_config';

interface WizardFooterProps {
    mode: Mode;
    onBack: () => void;
    onCreate: () => void;
    isCreating?: boolean;
}

export function WizardFooter({ mode, onBack, onCreate, isCreating = false }: WizardFooterProps) {
    if (mode === 'initial') return null;

    return (
        <div className="px-6 py-4 border-t border-border bg-muted flex justify-between items-center">
            <button
                onClick={onBack}
                disabled={isCreating}
                className="text-sm font-bold text-muted-foreground hover:text-card-foreground transition-colors flex items-center gap-1 disabled:opacity-50 disabled:cursor-not-allowed"
            >
                <span className="material-symbols-outlined text-[18px]">arrow_back</span>
                Back
            </button>
            <button
                onClick={onCreate}
                disabled={isCreating}
                className="px-6 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed disabled:hover:bg-primary"
            >
                {isCreating ? (
                    <>
                        <span className="inline-block w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></span>
                        Creating...
                    </>
                ) : (
                    <>
                        {mode === 'import_gitlab' ? 'Connect Repository' : 'Create Project'}
                        <span className="material-symbols-outlined text-[18px]">arrow_forward</span>
                    </>
                )}
            </button>
        </div>
    );
}
