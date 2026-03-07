const ABSOLUTE_LOCAL_PATH_PATTERN =
  /(?:\/Users\/|\/home\/|\/tmp\/|\/var\/folders\/)[^\s"',)]+/g;

const PATH_DISPLAY_MARKERS = [
  'src',
  'app',
  'frontend',
  'crates',
  'scripts',
  'tests',
  'docs',
  'public',
  '.acpms',
];

function splitPathSegments(value: string): string[] {
  return value.split('/').filter(Boolean);
}

function stripGitSuffix(value: string): string {
  return value.replace(/\.git$/i, '');
}

function formatSkillPlaybookPath(path: string): string | null {
  const segments = splitPathSegments(path);
  const skillsIndex = segments.lastIndexOf('skills');
  if (skillsIndex < 0) return null;

  let skillIndex = skillsIndex + 1;
  if (segments[skillIndex] === '.system') {
    skillIndex += 1;
  }

  const skillId = segments[skillIndex];
  if (!skillId) return null;

  const fileName = segments[segments.length - 1]?.toLowerCase();
  if (fileName === 'skill.md') {
    return `${skillId} skill playbook`;
  }
  return skillId;
}

export function formatLogPathForDisplay(rawPath: string): string {
  const path = String(rawPath || '').trim();
  if (!path) return '';
  if (!path.startsWith('/')) return path;

  const skillPath = formatSkillPlaybookPath(path);
  if (skillPath) {
    return skillPath;
  }

  const segments = splitPathSegments(path);
  if (segments.length === 0) return path;

  const markerIndex = segments.findIndex((segment) =>
    PATH_DISPLAY_MARKERS.includes(segment)
  );
  if (markerIndex >= 0) {
    return segments.slice(markerIndex).join('/');
  }

  const lastSegment = stripGitSuffix(segments[segments.length - 1]);
  if (!lastSegment) return path;

  const looksLikeFile = lastSegment.includes('.');
  if (!looksLikeFile) {
    return lastSegment;
  }

  return segments.slice(Math.max(0, segments.length - 2)).join('/');
}

export function formatLogPathForConversation(rawPath: string): string {
  const displayPath = formatLogPathForDisplay(rawPath);
  const segments = splitPathSegments(displayPath);
  const firstSegment = segments[0];
  const lastSegment = segments[segments.length - 1];

  if (
    segments.length === 2 &&
    lastSegment?.includes('.') &&
    firstSegment &&
    !PATH_DISPLAY_MARKERS.includes(firstSegment)
  ) {
    return lastSegment;
  }

  return displayPath;
}

export function humanizeLogText(text: string): string {
  return String(text || '').replace(ABSOLUTE_LOCAL_PATH_PATTERN, (match) =>
    formatLogPathForDisplay(match)
  );
}
