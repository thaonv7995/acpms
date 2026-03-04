// Parse stream-json format logs from Claude CLI

export interface StructuredLog {
  type: 'text' | 'tool_call' | 'tool_result' | 'thinking' | 'message' | 'error';
  content: any;
  metadata?: {
    tool_name?: string;
    tool_use_id?: string;
    model?: string;
    is_error?: boolean;
  };
}

/**
 * Parse JSON line from Claude CLI stream-json output
 *
 * Expected formats:
 * - {"type":"message","role":"assistant","content":"text"}
 * - {"type":"tool_use","id":"...","name":"Edit","input":{...}}
 * - {"type":"tool_result","tool_use_id":"...","content":"..."}
 * - {"type":"text","text":"..."}
 */
export function parseStructuredLog(jsonLine: string): StructuredLog {
  try {
    const data = JSON.parse(jsonLine);

    // Message type (assistant response)
    if (data.type === 'message') {
      const content = Array.isArray(data.content)
        ? data.content.map((c: any) => c.text || c).join('')
        : data.content;

      return {
        type: 'message',
        content,
        metadata: {
          model: data.model,
        },
      };
    }

    // Tool use (tool call)
    if (data.type === 'tool_use') {
      return {
        type: 'tool_call',
        content: {
          name: data.name,
          input: data.input,
        },
        metadata: {
          tool_name: data.name,
          tool_use_id: data.id,
        },
      };
    }

    // Tool result
    if (data.type === 'tool_result') {
      return {
        type: 'tool_result',
        content: data.content,
        metadata: {
          tool_use_id: data.tool_use_id,
          is_error: data.is_error,
        },
      };
    }

    // Thinking block
    if (data.type === 'thinking') {
      return {
        type: 'thinking',
        content: data.thinking || data.content,
      };
    }

    // Text chunk
    if (data.type === 'text') {
      return {
        type: 'text',
        content: data.text,
      };
    }

    // Error
    if (data.type === 'error') {
      return {
        type: 'error',
        content: data.message || data.error || JSON.stringify(data),
      };
    }

    // Unknown type - return as text
    return {
      type: 'text',
      content: JSON.stringify(data, null, 2),
    };
  } catch (e) {
    // Not valid JSON - return as plain text
    return {
      type: 'text',
      content: jsonLine,
    };
  }
}

/**
 * Format structured log for display
 */
export function formatStructuredLog(log: StructuredLog): string {
  switch (log.type) {
    case 'tool_call':
      return `🔧 Tool: ${log.content.name}\nInput: ${JSON.stringify(log.content.input, null, 2)}`;

    case 'tool_result':
      return `✅ Result: ${log.content}`;

    case 'thinking':
      return `💭 ${log.content}`;

    case 'message':
      return log.content;

    case 'error':
      return `❌ Error: ${log.content}`;

    default:
      return log.content;
  }
}
