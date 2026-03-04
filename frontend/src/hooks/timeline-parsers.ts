import type { TimelineEntry, ToolCallEntry } from '@/types/timeline-log';

export function mapStdoutToolAction(toolName: string): string {
    const normalized = toolName.toLowerCase();
    switch (normalized) {
        case 'read':
            return 'file_read';
        case 'edit':
            return 'file_edit';
        case 'write':
            return 'file_write';
        case 'bash':
            return 'command_run';
        case 'grep':
        case 'glob':
            return 'search';
        case 'todowrite':
        case 'todo_write':
        case 'todoread':
        case 'todo_read':
        case 'todo_management':
        case 'todomanagement':
            return 'todo_management';
        case 'webfetch':
        case 'web_fetch':
            return 'web_fetch';
        case 'task':
            return 'task_create';
        default:
            return 'tool';
    }
}

export function inferToolAction(toolName: string, actionType: Record<string, any>): string {
    const declared = typeof actionType.action === 'string' ? actionType.action : '';
    if (declared && declared !== 'tool') {
        return declared;
    }

    // Tool wrapper with richer payload should still render semantically.
    if (Array.isArray(actionType.todos)) return 'todo_management';
    if (typeof actionType.command === 'string' && actionType.command.trim()) return 'command_run';
    if (typeof actionType.path === 'string' && actionType.path.trim()) {
        if (Array.isArray(actionType.changes) && actionType.changes.length > 0) return 'file_edit';
        return mapStdoutToolAction(toolName);
    }
    if (typeof actionType.url === 'string' && actionType.url.trim()) return 'web_fetch';
    if (typeof actionType.query === 'string' && actionType.query.trim()) return 'search';
    if (typeof actionType.description === 'string' && actionType.description.trim()) return 'task_create';
    if (typeof actionType.tool_name === 'string' && actionType.tool_name.trim()) {
        const mapped = mapStdoutToolAction(actionType.tool_name);
        if (mapped !== 'tool') {
            return mapped;
        }
    }

    const args = actionType.arguments;
    if (args && typeof args === 'object') {
        if (Array.isArray((args as { todos?: unknown }).todos)) return 'todo_management';
        if (typeof (args as { command?: unknown }).command === 'string') return 'command_run';
        if (typeof (args as { file_path?: unknown }).file_path === 'string') {
            return mapStdoutToolAction(toolName);
        }
    }

    return mapStdoutToolAction(toolName);
}

export function inferToolTarget(action: string, actionType: Record<string, any>, toolName: string): string | null {
    if (action === 'todo_management') {
        return null;
    }

    const explicitTarget = actionType.path ||
        actionType.file_path ||
        actionType.target ||
        actionType.command ||
        actionType.query ||
        actionType.url ||
        actionType.description;

    if (typeof explicitTarget === 'string' && explicitTarget.trim()) {
        return explicitTarget;
    }

    const args = actionType.arguments;
    if (args && typeof args === 'object') {
        const argTarget = (args as any).file_path ||
            (args as any).path ||
            (args as any).target ||
            (args as any).command ||
            (args as any).query ||
            (args as any).url ||
            (args as any).description;
        if (typeof argTarget === 'string' && argTarget.trim()) {
            return argTarget;
        }
    }

    // Fallback map target for task creation if tool name was task
    if (action === 'task_create') {
        return toolName;
    }

    return null;
}

export function normalizeToolStatus(
    statusObj: Record<string, any> | null | undefined,
): {
    status:
    | 'created'
    | 'running'
    | 'pending_approval'
    | 'success'
    | 'failed'
    | 'denied'
    | 'timed_out'
    | 'cancelled';
    reason?: string;
    approvalId?: string;
} {
    const rawStatus = String(statusObj?.status || '').toLowerCase();

    if (rawStatus === 'success') return { status: 'success' };
    if (rawStatus === 'failed') return { status: 'failed' };
    if (rawStatus === 'cancelled' || rawStatus === 'canceled') return { status: 'cancelled' };
    if (rawStatus === 'timed_out' || rawStatus === 'timeout') return { status: 'timed_out' };
    if (rawStatus === 'denied') {
        return {
            status: 'denied',
            reason: statusObj?.reason ? String(statusObj.reason) : undefined,
        };
    }
    if (rawStatus === 'pending_approval') {
        return {
            status: 'pending_approval',
            approvalId:
                statusObj?.approval_id ? String(statusObj.approval_id) : undefined,
        };
    }
    if (rawStatus === 'running' || rawStatus === 'in_progress') {
        return { status: 'running' };
    }
    if (rawStatus === 'created') {
        // VK-like: treat "created" as an in-progress tool card (avoid hiding tool calls).
        return { status: 'running' };
    }

    return { status: 'success' };
}

export function splitConcatenatedJsonObjects(input: string): string[] {
    const text = input.trim();
    if (!text.startsWith('{')) return [];

    const results: string[] = [];
    let depth = 0;
    let inString = false;
    let escape = false;
    let start = -1;

    for (let i = 0; i < text.length; i += 1) {
        const ch = text[i];

        if (start === -1) {
            if (ch === '{') {
                start = i;
                depth = 1;
                inString = false;
                escape = false;
            }
            continue;
        }

        if (inString) {
            if (escape) {
                escape = false;
                continue;
            }
            if (ch === '\\') {
                escape = true;
                continue;
            }
            if (ch === '"') {
                inString = false;
            }
            continue;
        }

        if (ch === '"') {
            inString = true;
            continue;
        }
        if (ch === '{') {
            depth += 1;
            continue;
        }
        if (ch === '}') {
            depth -= 1;
            if (depth === 0) {
                results.push(text.slice(start, i + 1));
                start = -1;
            }
        }
    }

    return results;
}

export function parseCodexJsonStdoutEntries(
    content: string,
    log: any,
    index: number,
): TimelineEntry[] | null {
    const trimmed = (content || '').trim();
    if (!trimmed.startsWith('{')) return null;

    // Avoid hiding arbitrary JSON: only treat as Codex JSONL if the event shape matches.
    const looksLikeCodexEvent = /"type"\s*:\s*"(thread|turn|item)\./.test(trimmed);
    if (!looksLikeCodexEvent) return null;

    const baseId = log.id || `stdout-${index}`;
    const baseTimestamp = log.timestamp || log.created_at || new Date().toISOString();

    const objects = splitConcatenatedJsonObjects(trimmed);
    const chunks = objects.length > 0 ? objects : [trimmed];

    const entries: TimelineEntry[] = [];
    let parsedAny = false;

    for (let subIndex = 0; subIndex < chunks.length; subIndex += 1) {
        const chunk = chunks[subIndex];
        try {
            const parsed = JSON.parse(chunk);
            parsedAny = true;

            const eventType = typeof parsed?.type === 'string' ? parsed.type : '';
            if (!eventType) continue;

            // Ignore thread/turn lifecycle noise.
            if (eventType.startsWith('thread.') || eventType.startsWith('turn.')) {
                continue;
            }

            const item = parsed?.item;
            const itemType = typeof item?.type === 'string' ? item.type : '';

            // Ignore internal reasoning to keep the UI non-technical (Vibe Kanban-like).
            if (itemType === 'reasoning') {
                continue;
            }

            if (eventType === 'item.completed' && itemType === 'agent_message') {
                const text = typeof item?.text === 'string' ? item.text : '';
                if (!text.trim()) continue;

                entries.push({
                    id: `${baseId}-codex-msg-${item?.id || subIndex}`,
                    type: 'assistant_message',
                    timestamp: baseTimestamp,
                    content: text,
                    source: 'codex',
                });
                continue;
            }

            // Command execution (Codex emits /bin/zsh -lc ... commands).
            if (eventType === 'item.completed' && itemType === 'command_execution') {
                const command = typeof item?.command === 'string' ? item.command : '';
                if (!command.trim()) continue;

                const exitCodeRaw = item?.exit_code ?? item?.exitCode;
                const exitCode =
                    typeof exitCodeRaw === 'number' ? exitCodeRaw : null;
                const status =
                    exitCode === null
                        ? typeof item?.status === 'string' && /in_progress|running/i.test(item.status)
                            ? 'running'
                            : 'success'
                        : exitCode === 0
                            ? 'success'
                            : 'failed';

                const aggregatedOutput =
                    typeof item?.aggregated_output === 'string'
                        ? item.aggregated_output
                        : typeof item?.output === 'string'
                            ? item.output
                            : undefined;

                entries.push({
                    id: `${baseId}-codex-cmd-${item?.id || subIndex}`,
                    type: 'tool_call',
                    timestamp: baseTimestamp,
                    toolName: 'Bash',
                    actionType: {
                        action: 'command_run',
                        command,
                        target: command,
                        result: aggregatedOutput ? { output: aggregatedOutput } : undefined,
                    },
                    status,
                    statusReason:
                        status === 'failed' && typeof exitCode === 'number' ? `exit ${exitCode}` : undefined,
                });
                continue;
            }

            // File change events (create/update/delete)
            // Codex: item.changes = [{path, kind}]; Gemini: item.path + item.kind
            if (eventType === 'item.completed' && itemType === 'file_change') {
                const changes = Array.isArray(item?.changes) ? item.changes : [];
                const itemsToEmit =
                    changes.length > 0
                        ? changes.map((c: any) => ({
                              path: typeof c?.path === 'string' ? c.path : '',
                              kind: (c?.kind ?? '').toString(),
                          }))
                        : typeof item?.path === 'string'
                          ? [{ path: item.path, kind: (item?.change_type ?? item?.kind ?? '').toString() }]
                          : [];
                for (let i = 0; i < itemsToEmit.length; i++) {
                    const { path, kind } = itemsToEmit[i];
                    if (!path.trim()) continue;
                    const kindRaw = kind.toLowerCase();
                    const changeType: any =
                        kindRaw.includes('create') || kindRaw.includes('add')
                            ? 'Created'
                            : kindRaw.includes('delete') || kindRaw.includes('remove')
                                ? 'Deleted'
                                : 'Modified';
                    entries.push({
                        id: `${baseId}-codex-file-${item?.id || subIndex}-${i}`,
                        type: 'file_change',
                        timestamp: baseTimestamp,
                        path,
                        changeType,
                        linesAdded: typeof item?.lines_added === 'number' ? item.lines_added : undefined,
                        linesRemoved: typeof item?.lines_removed === 'number' ? item.lines_removed : undefined,
                    });
                }
                continue;
            }
        } catch {
            // Ignore parse failures; we'll fall back if we never parsed anything.
        }
    }

    // If we successfully parsed Codex JSON events, hide the raw JSON line even if it mapped to no UI entries.
    if (parsedAny) return entries;
    return null;
}

export function formatShellCommandForDisplay(command: string): string {
    let cmd = (command || '').trim();
    if (!cmd) return cmd;

    const lcIndex = cmd.indexOf(' -lc ');
    if (lcIndex !== -1) {
        cmd = cmd.slice(lcIndex + 5).trim();
        if (
            (cmd.startsWith("'") && cmd.endsWith("'")) ||
            (cmd.startsWith('"') && cmd.endsWith('"'))
        ) {
            cmd = cmd.slice(1, -1);
        } else if (cmd.startsWith("'") || cmd.startsWith('"')) {
            cmd = cmd.slice(1);
        }
        cmd = cmd.trim();
    }

    // Strip the boilerplate "cd <path> &&" prefix used by some agents.
    cmd = cmd.replace(/^cd\s+[^&]+&&\s*/i, '');

    return cmd.trim();
}

export function parseStdoutTranscriptEntries(
    content: string,
    log: any,
    index: number,
): TimelineEntry[] | null {
    const text = (content || '').trim();
    if (!text) return null;

    const hasMarkers =
        text.includes('Using tool:') ||
        text.includes('✓') ||
        text.includes('✗') ||
        /\b(Created|Modified|Deleted|Renamed):\s+/.test(text);
    if (!hasMarkers) return null;

    const baseId = log.id || `stdout-${index}`;
    const baseTimestamp = log.timestamp || log.created_at || new Date().toISOString();

    // Matches the *start* of a marker. We compute the full segment by scanning to the next marker.
    const markerStartRegex =
        /(Using tool:\s+\w+)|([✓✗]\s+\w+\s+(?:completed|failed|cancelled))|((?:Created|Modified|Deleted|Renamed):\s+[^\s]+)/g;

    const entries: TimelineEntry[] = [];
    let parsedAny = false;
    let cursor = 0;

    const pushAssistantText = (chunk: string) => {
        const trimmed = chunk.trim();
        if (!trimmed) return;
        entries.push({
            id: `${baseId}-text-${entries.length}`,
            type: 'assistant_message',
            timestamp: baseTimestamp,
            content: trimmed,
            source: 'stdout',
        });
    };

    const updateLastToolStatus = (toolName: string, status: ToolCallEntry['status'], reason?: string) => {
        // Agent runs commands sequentially: "✓ Bash completed" finishes the OLDEST still-running Bash.
        // Search from start to find the first matching running entry.
        for (let i = 0; i < entries.length; i += 1) {
            const entry = entries[i];
            if (entry.type !== 'tool_call') continue;
            const toolCall = entry as ToolCallEntry;
            if ((toolCall.toolName || '').toLowerCase() !== toolName.toLowerCase()) continue;
            if (toolCall.status && toolCall.status !== 'running' && status === 'success') {
                continue;
            }
            toolCall.status = status;
            if (reason) {
                toolCall.statusReason = reason;
            }
            return;
        }
    };

    while (cursor < text.length) {
        markerStartRegex.lastIndex = cursor;
        const match = markerStartRegex.exec(text);
        if (!match) {
            pushAssistantText(text.slice(cursor));
            break;
        }

        parsedAny = true;
        const markerIndex = match.index;
        if (markerIndex > cursor) {
            pushAssistantText(text.slice(cursor, markerIndex));
        }

        const markerToken = match[0];
        const segmentStart = markerIndex;
        const searchFrom = segmentStart + markerToken.length;

        // Find the start of the next marker without consuming it.
        markerStartRegex.lastIndex = searchFrom;
        const next = markerStartRegex.exec(text);
        const segmentEnd = next ? next.index : text.length;

        // Reset to allow re-reading the next marker on the next iteration.
        if (next) {
            markerStartRegex.lastIndex = next.index;
        }

        const segment = text.slice(segmentStart, segmentEnd).trim();

        // Tool start: "Using tool: Bash <payload>"
        if (segment.startsWith('Using tool:')) {
            const toolMatch = /^Using tool:\s+(\w+)\s*/.exec(segment);
            const toolName = toolMatch?.[1] || 'Tool';
            const payload = segment.slice(toolMatch?.[0].length || 0).trim();

            const action = mapStdoutToolAction(toolName);
            const normalizedPayload =
                toolName.toLowerCase() === 'bash' ? formatShellCommandForDisplay(payload) : payload;

            entries.push({
                id: `${baseId}-tool-${entries.length}`,
                type: 'tool_call',
                timestamp: baseTimestamp,
                toolName,
                actionType: {
                    action,
                    target: normalizedPayload || payload || segment,
                    command: action === 'command_run' ? normalizedPayload || payload : undefined,
                    query: action === 'search' ? normalizedPayload || payload : undefined,
                    url: action === 'web_fetch' ? normalizedPayload || payload : undefined,
                    file_path: action.startsWith('file_') ? normalizedPayload || payload : undefined,
                    path: action.startsWith('file_') ? normalizedPayload || payload : undefined,
                },
                status: 'running',
            });

            cursor = segmentEnd;
            continue;
        }

        // Tool status: "✓ Bash completed" / "✗ Bash failed: ..."
        const statusMatch =
            /^([✓✗])\s+(\w+)\s+(completed|failed|cancelled)(?::\s*(.+))?/i.exec(segment);
        if (statusMatch) {
            const symbol = statusMatch[1];
            const toolName = statusMatch[2];
            const verb = statusMatch[3].toLowerCase();
            const reason = statusMatch[4]?.trim();

            const status: ToolCallEntry['status'] =
                verb === 'cancelled'
                    ? 'cancelled'
                    : symbol === '✗' || verb === 'failed'
                        ? 'failed'
                        : 'success';

            updateLastToolStatus(toolName, status, reason);
            cursor = segmentEnd;
            continue;
        }

        // File change: "Modified: README.md (+15, -2)"
        const fileMatch =
            /^(Created|Modified|Deleted|Renamed):\s+([^\s]+)(?:\s+\(\+(\d+),\s*-(\d+)\))?/i.exec(segment);
        if (fileMatch) {
            const changeStr = fileMatch[1].toLowerCase();
            const path = fileMatch[2];
            const changeType =
                changeStr === 'created'
                    ? 'Created'
                    : changeStr === 'deleted'
                        ? 'Deleted'
                        : 'Modified';
            const linesAdded = fileMatch[3] ? Number(fileMatch[3]) : undefined;
            const linesRemoved = fileMatch[4] ? Number(fileMatch[4]) : undefined;

            entries.push({
                id: `${baseId}-file-${entries.length}`,
                type: 'file_change',
                timestamp: baseTimestamp,
                path,
                changeType,
                linesAdded,
                linesRemoved,
            });

            cursor = segmentEnd;
            continue;
        }

        // Unknown marker: treat as assistant text to avoid dropping content.
        pushAssistantText(segment);
        cursor = segmentEnd;
    }

    if (parsedAny) return entries;
    return null;
}
