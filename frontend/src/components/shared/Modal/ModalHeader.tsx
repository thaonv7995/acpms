interface ModalHeaderProps {
  title: string;
  icon?: string;
  iconColor?: string;
  onClose?: () => void;
}

export function ModalHeader({ title, icon, iconColor, onClose }: ModalHeaderProps) {
  return (
    <div className="flex items-center justify-between mb-4">
      <div className="flex items-center gap-3">
        {icon && (
          <div className={`p-2 rounded-lg ${iconColor || 'bg-primary/10 text-primary'}`}>
            <span className="material-symbols-outlined">{icon}</span>
          </div>
        )}
        <h2 className="text-lg font-bold text-slate-900 dark:text-white">{title}</h2>
      </div>
      {onClose && (
        <button
          onClick={onClose}
          className="p-1 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
          aria-label="Close"
        >
          <span className="material-symbols-outlined text-slate-500 dark:text-slate-400">
            close
          </span>
        </button>
      )}
    </div>
  );
}
