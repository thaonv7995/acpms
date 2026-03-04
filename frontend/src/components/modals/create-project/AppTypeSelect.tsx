type AppType = 'web' | 'mobile' | 'desktop' | 'extension';

interface AppTypeSelectProps {
    onSelectType: (type: AppType) => void;
}

export function AppTypeSelect({ onSelectType }: AppTypeSelectProps) {
    return (
        <div className="grid grid-cols-2 gap-4">
            {[
                { id: 'web', label: 'Web Application', icon: 'language', desc: 'React, Vue, Angular, etc.' },
                { id: 'mobile', label: 'Mobile App', icon: 'smartphone', desc: 'React Native, Flutter, Swift' },
                { id: 'desktop', label: 'Desktop App', icon: 'desktop_windows', desc: 'Electron, Tauri, .NET' },
                { id: 'extension', label: 'Browser Extension', icon: 'extension', desc: 'Chrome, Firefox Add-ons' },
            ].map((type) => (
                <button
                    key={type.id}
                    onClick={() => onSelectType(type.id as AppType)}
                    className="flex flex-col items-start p-4 rounded-xl bg-white dark:bg-[#161b22] border border-slate-200 dark:border-slate-700 hover:border-primary dark:hover:border-primary hover:bg-slate-50 dark:hover:bg-[#1c2128] transition-all text-left"
                >
                    <span className="material-symbols-outlined text-slate-500 dark:text-slate-400 text-3xl mb-3">{type.icon}</span>
                    <h4 className="font-bold text-slate-900 dark:text-white">{type.label}</h4>
                    <p className="text-xs text-slate-500 dark:text-slate-400 mt-1">{type.desc}</p>
                </button>
            ))}
        </div>
    );
}
