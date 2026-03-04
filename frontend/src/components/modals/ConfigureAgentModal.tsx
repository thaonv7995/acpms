import { useState } from 'react';

interface ConfigureAgentModalProps {
    isOpen: boolean;
    onClose: () => void;
    taskId: string;
    taskTitle: string;
    onStart?: (config: { agent: string; strategy: string; attemptLimit: number }) => void;
}

export function ConfigureAgentModal({ isOpen, onClose, taskId, taskTitle, onStart }: ConfigureAgentModalProps) {
    const [selectedAgent, setSelectedAgent] = useState<'claude' | 'gpt4' | 'gemini'>('claude');
    const [attemptLimit, setAttemptLimit] = useState(3);
    const [strategy, setStrategy] = useState('Chain of Thought (Reasoning)');

    if (!isOpen) return null;

    const handleStart = () => {
        if (onStart) {
            onStart({ agent: selectedAgent, strategy, attemptLimit });
        }
        onClose();
    };

    return (
        <div className="fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-6 font-display">
            <div className="absolute inset-0 bg-black/70 backdrop-blur-[2px] transition-opacity" onClick={onClose}></div>
            <div className="relative w-full max-w-3xl bg-[#0d1117] border border-slate-700/80 rounded-2xl shadow-2xl overflow-hidden flex flex-col max-h-[90vh]">
                {/* Header */}
                <div className="px-6 py-5 border-b border-slate-800 flex justify-between items-start bg-[#161b22]">
                    <div>
                        <h2 className="text-lg font-bold text-white flex items-center gap-2">
                            <span className="material-symbols-outlined text-primary">smart_toy</span>
                            Configure Agent Execution
                        </h2>
                        <p className="text-sm text-slate-400 mt-1 font-mono">
                            Initiate new attempt for <span className="text-slate-200 font-semibold">{taskId}:</span> <span className="text-slate-300 font-mono text-xs">{taskTitle}</span>
                        </p>
                    </div>
                    <button onClick={onClose} className="text-slate-500 hover:text-white transition-colors">
                        <span className="material-symbols-outlined">close</span>
                    </button>
                </div>

                {/* Body */}
                <div className="p-6 overflow-y-auto bg-[#0d1117]">
                    {/* Agent Selection */}
                    <div className="mb-8">
                        <h3 className="text-[11px] font-bold text-slate-500 uppercase tracking-wider mb-3">Select Agent Profile</h3>
                        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
                            {/* Claude Card */}
                            <div
                                onClick={() => setSelectedAgent('claude')}
                                className={`relative p-4 rounded-xl border cursor-pointer transition-all group ${selectedAgent === 'claude' ? 'border-primary bg-primary/5 ring-1 ring-primary' : 'border-slate-700 bg-[#161b22] hover:border-slate-500'}`}
                            >
                                {selectedAgent === 'claude' && (
                                    <div className="absolute top-2 right-2 text-primary">
                                        <span className="material-symbols-outlined text-[20px] material-symbols-filled">check_circle</span>
                                    </div>
                                )}
                                <div className="size-8 rounded bg-[#D97757] flex items-center justify-center text-white font-bold text-lg mb-3 shadow-lg shadow-[#D97757]/20">C</div>
                                <h4 className="font-bold text-white mb-1 text-sm">Claude 3.5 Sonnet</h4>
                                <p className="text-[11px] text-slate-400 mb-3 leading-relaxed">Best for coding & complex reasoning.</p>
                                <p className="text-[10px] font-mono text-[#0bda5b] font-medium">$0.02/run</p>
                            </div>

                            {/* GPT-4o Card */}
                            <div
                                onClick={() => setSelectedAgent('gpt4')}
                                className={`relative p-4 rounded-xl border cursor-pointer transition-all group ${selectedAgent === 'gpt4' ? 'border-primary bg-primary/5 ring-1 ring-primary' : 'border-slate-700 bg-[#161b22] hover:border-slate-500'}`}
                            >
                                {selectedAgent === 'gpt4' && (
                                    <div className="absolute top-2 right-2 text-primary">
                                        <span className="material-symbols-outlined text-[20px] material-symbols-filled">check_circle</span>
                                    </div>
                                )}
                                <div className="size-8 rounded bg-[#10a37f] flex items-center justify-center text-white font-bold text-lg mb-3 shadow-lg shadow-[#10a37f]/20">G</div>
                                <h4 className="font-bold text-white mb-1 text-sm">GPT-4o</h4>
                                <p className="text-[11px] text-slate-400 mb-3 leading-relaxed">High speed, general purpose.</p>
                                <p className="text-[10px] font-mono text-[#0bda5b] font-medium">$0.01/run</p>
                            </div>

                            {/* Gemini Card */}
                            <div
                                onClick={() => setSelectedAgent('gemini')}
                                className={`relative p-4 rounded-xl border cursor-pointer transition-all group ${selectedAgent === 'gemini' ? 'border-primary bg-primary/5 ring-1 ring-primary' : 'border-slate-700 bg-[#161b22] hover:border-slate-500'}`}
                            >
                                {selectedAgent === 'gemini' && (
                                    <div className="absolute top-2 right-2 text-primary">
                                        <span className="material-symbols-outlined text-[20px] material-symbols-filled">check_circle</span>
                                    </div>
                                )}
                                <div className="size-8 rounded bg-[#4b9eff] flex items-center justify-center text-white font-bold text-lg mb-3 shadow-lg shadow-[#4b9eff]/20">G</div>
                                <h4 className="font-bold text-white mb-1 text-sm">Gemini 1.5 Pro</h4>
                                <p className="text-[11px] text-slate-400 mb-3 leading-relaxed">Huge context window (1M+ tokens).</p>
                                <p className="text-[10px] font-mono text-[#0bda5b] font-medium">$0.005/run</p>
                            </div>
                        </div>
                    </div>

                    {/* Config Row */}
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-8 mb-8">
                        <div>
                            <h3 className="text-[11px] font-bold text-slate-500 uppercase tracking-wider mb-2">Execution Strategy</h3>
                            <div className="relative">
                                <select
                                    value={strategy}
                                    onChange={(e) => setStrategy(e.target.value)}
                                    className="w-full bg-[#161b22] border border-slate-700 text-slate-200 text-sm rounded-lg p-2.5 pr-8 appearance-none focus:ring-1 focus:ring-primary focus:border-primary outline-none hover:border-slate-600 transition-colors"
                                >
                                    <option>Chain of Thought (Reasoning)</option>
                                    <option>ReAct (Reason + Act)</option>
                                    <option>Code Generation Only</option>
                                </select>
                                <span className="absolute right-2.5 top-3 text-slate-400 pointer-events-none">
                                    <span className="material-symbols-outlined text-sm">expand_more</span>
                                </span>
                            </div>
                        </div>
                        <div>
                            <div className="flex justify-between mb-3">
                                <h3 className="text-[11px] font-bold text-slate-500 uppercase tracking-wider">Attempt Limit</h3>
                                <span className="text-xs font-bold text-white bg-slate-800 px-2 py-0.5 rounded">{attemptLimit}</span>
                            </div>
                            <div className="flex items-center gap-4">
                                <input
                                    type="range"
                                    min="1"
                                    max="5"
                                    value={attemptLimit}
                                    onChange={(e) => setAttemptLimit(parseInt(e.target.value))}
                                    className="w-full h-1.5 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-primary"
                                />
                            </div>
                            <p className="text-[10px] text-slate-500 mt-2">Max retries on error.</p>
                        </div>
                    </div>

                    {/* Context Included */}
                    <div className="p-4 rounded-xl bg-[#161b22] border border-slate-800/60">
                        <div className="flex items-center gap-2 mb-3">
                            <span className="material-symbols-outlined text-slate-400 text-sm">description</span>
                            <h3 className="text-[11px] font-bold text-white">Context Included</h3>
                        </div>
                        <div className="flex flex-wrap gap-2">
                            <span className="bg-[#0d1117] border border-slate-700 text-slate-400 text-xs px-2.5 py-1.5 rounded-md font-mono">project-config.json</span>
                            <span className="bg-[#0d1117] border border-slate-700 text-slate-400 text-xs px-2.5 py-1.5 rounded-md font-mono">src/api/</span>
                            <span className="bg-[#0d1117] border border-slate-700 text-slate-400 text-xs px-2.5 py-1.5 rounded-md font-mono">relevant-files</span>
                        </div>
                    </div>
                </div>

                {/* Footer */}
                <div className="px-6 py-4 border-t border-slate-800 bg-[#161b22] flex justify-end gap-3">
                    <button onClick={onClose} className="px-4 py-2 text-sm font-medium text-slate-300 hover:text-white transition-colors">
                        Cancel
                    </button>
                    <button onClick={handleStart} className="px-5 py-2 bg-primary hover:bg-primary/90 text-primary-foreground text-sm font-bold rounded-lg shadow-lg shadow-primary/20 flex items-center gap-2 transition-all active:scale-95 hover:shadow-primary/40">
                        <span className="material-symbols-outlined text-[18px] material-symbols-filled">play_arrow</span>
                        Start Agent
                    </button>
                </div>
            </div>
        </div>
    );
}
