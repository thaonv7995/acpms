/**
 * StepSelectMethod - Step 1: Choose Creation Method
 *
 * Two options:
 * - From Scratch: Start with empty repo, agent scaffolds project
 * - Import from GitLab or GitHub: Clone existing repository
 */

export type CreationMethod = 'scratch' | 'gitlab';

interface StepSelectMethodProps {
  onSelectMethod: (method: CreationMethod) => void;
}

interface MethodOption {
  id: CreationMethod;
  title: string;
  description: string;
  icon: string;
  iconColor: string;
  bgColor: string;
}

const methodOptions: MethodOption[] = [
  {
    id: 'scratch',
    title: 'From Scratch',
    description: 'Start fresh. AI Agent will scaffold your project based on your requirements.',
    icon: 'add_circle',
    iconColor: 'text-primary',
    bgColor: 'bg-primary/10',
  },
  {
    id: 'gitlab',
    title: 'Import from GitLab or GitHub',
    description: 'Connect an existing repository. If it is read-only, ACPMS can fork it to your account for GitOps.',
    icon: 'code',
    iconColor: 'text-[#FC6D26]',
    bgColor: 'bg-[#FC6D26]/10',
  },
];

export function StepSelectMethod({ onSelectMethod }: StepSelectMethodProps) {
  return (
    <div className="space-y-4">
      <p className="text-sm text-muted-foreground mb-6">
        Choose how you want to start your project. You can always import code or apply templates later.
      </p>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {methodOptions.map((option) => (
          <button
            key={option.id}
            onClick={() => onSelectMethod(option.id)}
            className="group flex flex-col items-center p-6 rounded-xl bg-card border border-border hover:border-primary hover:bg-muted transition-all text-center"
          >
            <div
              className={`size-14 rounded-full ${option.bgColor} flex items-center justify-center mb-4 group-hover:scale-110 transition-transform`}
            >
              <span className={`material-symbols-outlined ${option.iconColor} text-3xl`}>
                {option.icon}
              </span>
            </div>
            <h3 className="text-base font-bold text-card-foreground mb-2">
              {option.title}
            </h3>
            <p className="text-xs text-muted-foreground leading-relaxed">
              {option.description}
            </p>
          </button>
        ))}
      </div>

      {/* Quick tips */}
      <div className="mt-6 p-4 rounded-lg bg-muted border border-border">
        <div className="flex items-start gap-3">
          <span className="material-symbols-outlined text-primary text-lg mt-0.5">tips_and_updates</span>
          <div>
            <h4 className="text-sm font-bold text-card-foreground mb-1">Quick Tip</h4>
            <p className="text-xs text-muted-foreground">
              If you're starting a new project, "From Scratch" lets the AI agent set up the optimal project structure.
              For existing codebases, use "Import from GitLab or GitHub" to connect your repository.
            </p>
          </div>
        </div>
      </div>
    </div>
  );
}

export default StepSelectMethod;
