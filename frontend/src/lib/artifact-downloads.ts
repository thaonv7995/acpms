export interface ArtifactDownloadRef {
  attemptId?: string;
  artifactId?: string;
  artifactKey?: string;
  artifactType?: string;
  os?: string;
  label: string;
  legacyUrl?: string;
  sizeBytes?: number | null;
  createdAt?: string;
}

export function extractArtifactDownloadRefs(
  metadata?: Record<string, unknown>,
  defaultAttemptId?: string
): ArtifactDownloadRef[] {
  if (!metadata) return [];

  const appDownloads = Array.isArray(metadata.app_downloads)
    ? metadata.app_downloads
    : [];

  const refs = appDownloads.reduce<ArtifactDownloadRef[]>((acc, item) => {
      if (!item || typeof item !== 'object') return acc;
      const entry = item as Record<string, unknown>;
      const legacyUrl =
        typeof entry.presigned_url === 'string'
          ? entry.presigned_url
          : typeof entry.url === 'string'
            ? entry.url
            : undefined;
      const artifactId =
        typeof entry.artifact_id === 'string' ? entry.artifact_id : undefined;
      const attemptId =
        typeof entry.attempt_id === 'string' ? entry.attempt_id : defaultAttemptId;

      if (!artifactId && !legacyUrl) {
        return acc;
      }

      acc.push({
        attemptId,
        artifactId,
        artifactKey:
          typeof entry.artifact_key === 'string' ? entry.artifact_key : undefined,
        artifactType:
          typeof entry.artifact_type === 'string' ? entry.artifact_type : undefined,
        os: typeof entry.os === 'string' ? entry.os : undefined,
        label: typeof entry.label === 'string' ? entry.label : 'Download',
        legacyUrl,
        sizeBytes:
          typeof entry.size_bytes === 'number' ? entry.size_bytes : null,
        createdAt:
          typeof entry.created_at === 'string' ? entry.created_at : undefined,
      } satisfies ArtifactDownloadRef);
      return acc;
    }, []);

  if (refs.length > 0) {
    return refs;
  }

  if (typeof metadata.app_download_url === 'string' && metadata.app_download_url) {
    return [
      {
        attemptId: defaultAttemptId,
        label: 'Download',
        legacyUrl: metadata.app_download_url,
      },
    ];
  }

  return [];
}
