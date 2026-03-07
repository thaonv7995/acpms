function stripOuterQuotes(value: string): string {
  if (
    (value.startsWith("'") && value.endsWith("'")) ||
    (value.startsWith('"') && value.endsWith('"'))
  ) {
    return value.slice(1, -1).trim();
  }

  if (value.startsWith("'") || value.startsWith('"')) {
    return value.slice(1).trim();
  }

  return value;
}

export function formatShellCommandForDisplay(command: string): string {
  let cmd = String(command || '').trim();
  if (!cmd) return cmd;

  const shellWrapperMatch = cmd.match(
    /^(?:env\b(?:\s+[A-Za-z_][A-Za-z0-9_]*=(?:"[^"]*"|'[^']*'|[^\s]+))*\s+)?(?:(?:\/usr\/bin\/env)\s+)?(?:(?:\/bin\/)?(?:bash|zsh|sh))\s+-(?:l)?c\s+([\s\S]+)$/i
  );

  if (shellWrapperMatch?.[1]) {
    cmd = stripOuterQuotes(shellWrapperMatch[1].trim());
  }

  // Strip the boilerplate "cd <path> &&" prefix used by some agents.
  cmd = cmd.replace(/^cd\s+[^&]+&&\s*/i, '');

  return cmd.trim();
}
