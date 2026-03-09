// PA-106: Floating chat button for Project Assistant
interface FloatingChatButtonProps {
  onClick: () => void;
  className?: string;
}

export function FloatingChatButton({ onClick, className }: FloatingChatButtonProps) {
  const placementClass =
    className?.trim() ||
    'fixed right-4 bottom-[calc(env(safe-area-inset-bottom)+1rem)] md:right-6 md:bottom-6';
  return (
    <button
      onClick={onClick}
      aria-label="Open Project Assistant"
      className={`${placementClass} z-[9999] inline-flex h-14 w-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg hover:bg-primary/90 transition-colors hover:scale-105 focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2`}
    >
      <span className="material-symbols-outlined text-[28px]">auto_awesome</span>
    </button>
  );
}
