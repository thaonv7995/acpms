// LiveAgentActivity Component - Real-time agent log streaming panel
import { useState, useRef, useEffect } from 'react';
import { useProjectAgentLogs } from '../../hooks/useProjectAgentLogs';

interface LiveAgentActivityProps {
    projectId: string;
}

export function LiveAgentActivity({ projectId }: LiveAgentActivityProps) {
    const logsContainerRef = useRef<HTMLDivElement>(null);
    const [autoScroll, setAutoScroll] = useState(true);
    const [selectedAgent, setSelectedAgent] = useState<string | null>(null);

    // Use real-time agent logs hook
    const { logs = [], activeAgents = [], isConnected, clearLogs } = useProjectAgentLogs(projectId);

    // Filter logs by selected agent (with defensive check)
    const filteredLogs = selectedAgent && logs
        ? logs.filter(log => log.task_id === selectedAgent)
        : (logs || []);

    // Auto-scroll to bottom when new logs arrive
    useEffect(() => {
        if (autoScroll && logsContainerRef.current) {
            logsContainerRef.current.scrollTop = logsContainerRef.current.scrollHeight;
        }
    }, [filteredLogs, autoScroll]);

    // Handle scroll to detect if user scrolled up
    const handleScroll = () => {
        if (logsContainerRef.current) {
            const { scrollTop, scrollHeight, clientHeight } = logsContainerRef.current;
            const isAtBottom = scrollHeight - scrollTop - clientHeight < 50;
            setAutoScroll(isAtBottom);
        }
    };

    // Format timestamp for display
    const formatTime = (timestamp: string) => {
        const date = new Date(timestamp);
        return date.toLocaleTimeString('en-US', { hour12: false });
    };

    // Get color for agent/task
    const getAgentColor = (taskId: string) => {
        const colors = ['text-blue-400', 'text-green-400', 'text-purple-400', 'text-orange-400', 'text-pink-400'];
        const index = activeAgents.findIndex(a => a.task_id === taskId);
        return colors[index % colors.length];
    };

    return (
        <div className="bg-white dark:bg-surface-dark border border-slate-200 dark:border-slate-700 rounded-xl overflow-hidden flex flex-col h-[400px] relative">
            {/* Header */}
            <div className="px-4 py-3 border-b border-slate-200 dark:border-slate-700 bg-slate-50 dark:bg-[#1a232c] flex justify-between items-center">
                <h3 className="text-sm font-bold text-slate-900 dark:text-white flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary text-lg">terminal</span>
                    Live Agent Activity
                    {activeAgents.length > 0 && (
                        <span className="text-xs bg-primary/20 text-primary px-1.5 py-0.5 rounded">
                            {activeAgents.length} active
                        </span>
                    )}
                </h3>
                <div className="flex items-center gap-2">
                    {logs.length > 0 && (
                        <button
                            onClick={clearLogs}
                            className="text-xs text-slate-400 hover:text-slate-600 dark:hover:text-slate-300"
                            title="Clear logs"
                        >
                            <span className="material-symbols-outlined text-sm">delete</span>
                        </button>
                    )}
                    <span className={`flex h-2 w-2 relative ${isConnected ? '' : 'opacity-50'}`}>
                        <span className={`animate-ping absolute inline-flex h-full w-full rounded-full ${isConnected ? 'bg-green-400' : 'bg-yellow-400'} opacity-75`}></span>
                        <span className={`relative inline-flex rounded-full h-2 w-2 ${isConnected ? 'bg-green-500' : 'bg-yellow-500'}`}></span>
                    </span>
                </div>
            </div>

            {/* Agent Filter Tabs (when multiple agents) */}
            {activeAgents.length > 1 && (
                <div className="px-2 py-1.5 border-b border-slate-200 dark:border-slate-700 bg-slate-50/50 dark:bg-[#1a232c]/50 flex gap-1 overflow-x-auto">
                    <button
                        onClick={() => setSelectedAgent(null)}
                        className={`px-2 py-1 text-[10px] font-medium rounded transition-colors whitespace-nowrap ${
                            selectedAgent === null
                                ? 'bg-primary text-primary-foreground'
                                : 'bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-300 dark:hover:bg-slate-600'
                        }`}
                    >
                        All ({logs.length})
                    </button>
                    {activeAgents.map((agent) => {
                        const agentLogs = logs.filter(l => l.task_id === agent.task_id);
                        return (
                            <button
                                key={agent.task_id}
                                onClick={() => setSelectedAgent(agent.task_id)}
                                className={`px-2 py-1 text-[10px] font-medium rounded transition-colors whitespace-nowrap truncate max-w-[120px] ${
                                    selectedAgent === agent.task_id
                                        ? 'bg-primary text-primary-foreground'
                                        : 'bg-slate-200 dark:bg-slate-700 text-slate-600 dark:text-slate-300 hover:bg-slate-300 dark:hover:bg-slate-600'
                                }`}
                                title={agent.task_title}
                            >
                                {agent.task_title.slice(0, 15)}... ({agentLogs.length})
                            </button>
                        );
                    })}
                </div>
            )}

            {/* Logs Container */}
            <div
                ref={logsContainerRef}
                onScroll={handleScroll}
                className="flex-1 overflow-y-auto p-3 font-mono text-xs bg-slate-900 dark:bg-[#0d1117]"
            >
                {filteredLogs.length === 0 ? (
                    <p className="text-slate-500">
                        {activeAgents.length === 0
                            ? 'No active agents. Start a task to see live logs.'
                            : 'Waiting for agent output...'}
                    </p>
                ) : (
                    <div className="space-y-0.5">
                        {filteredLogs.map((log) => (
                            <div key={log.id} className="flex gap-2 leading-relaxed">
                                <span className="text-slate-600 shrink-0">{formatTime(log.timestamp)}</span>
                                {activeAgents.length > 1 && !selectedAgent && (
                                    <span className={`shrink-0 ${getAgentColor(log.task_id)}`}>
                                        [{log.task_title.slice(0, 10)}]
                                    </span>
                                )}
                                {log.type === 'Log' ? (
                                    <span className={log.log_type === 'stderr' ? 'text-red-400' : 'text-slate-300'}>
                                        {log.content}
                                    </span>
                                ) : (
                                    <span className="text-yellow-400">
                                        Status: {log.status}
                                    </span>
                                )}
                            </div>
                        ))}
                    </div>
                )}
            </div>

            {/* Auto-scroll indicator */}
            {!autoScroll && logs.length > 0 && (
                <button
                    onClick={() => {
                        setAutoScroll(true);
                        if (logsContainerRef.current) {
                            logsContainerRef.current.scrollTop = logsContainerRef.current.scrollHeight;
                        }
                    }}
                    className="absolute bottom-14 right-4 px-2 py-1 bg-primary text-primary-foreground text-xs rounded shadow-lg"
                >
                    ↓ New logs
                </button>
            )}

            {/* Command Input */}
            <div className="p-2 bg-slate-50 dark:bg-[#1a232c] border-t border-slate-200 dark:border-slate-700">
                <input
                    className="w-full bg-transparent border-none text-xs font-mono focus:ring-0 text-slate-900 dark:text-white placeholder:text-slate-500"
                    placeholder={activeAgents.length > 0 ? "Send command to agent..." : "No active agents"}
                    disabled={activeAgents.length === 0}
                    type="text"
                />
            </div>
        </div>
    );
}
