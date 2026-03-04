interface PreviewSectionProps {
    previewUrl?: string;
    appDownloadUrl?: string;
    appDownloads?: Array<Record<string, unknown>>;
    previewTarget?: string;
    deploymentStatus?: string;
    deploymentError?: string;
    appVersion?: string;
    isCompleted?: boolean;
}

export function PreviewSection({
    previewUrl,
    appDownloadUrl,
    appDownloads = [],
    previewTarget,
    deploymentStatus,
    deploymentError,
    appVersion,
    isCompleted = false,
}: PreviewSectionProps) {
    const normalizedDownloads = appDownloads
        .map((item) => {
            const label = typeof item.label === 'string' ? item.label : 'Download';
            const os = typeof item.os === 'string' ? item.os : 'generic';
            const url = typeof item.url === 'string' ? item.url : undefined;
            const presignedUrl =
                typeof item.presigned_url === 'string' ? item.presigned_url : undefined;
            const artifactType =
                typeof item.artifact_type === 'string' ? item.artifact_type : undefined;
            return {
                label,
                os,
                artifactType,
                url: presignedUrl || url,
            };
        })
        .filter((item) => Boolean(item.url));

    const downloadEntries =
        normalizedDownloads.length > 0
            ? normalizedDownloads
            : appDownloadUrl
              ? [
                    {
                        label: 'Download',
                        os: 'generic',
                        artifactType: undefined,
                        url: appDownloadUrl,
                    },
                ]
              : [];

    // Don't render if no preview, no downloads, and no deployment status/error.
    if (!previewUrl && downloadEntries.length === 0 && !deploymentError && !deploymentStatus) {
        return null;
    }

    return (
        <div className="bg-card border border-border rounded-xl p-6">
            <h3 className="text-xs font-bold text-card-foreground uppercase tracking-wider mb-4 flex items-center gap-2">
                <span className="material-symbols-outlined text-[16px] text-muted-foreground">
                    {isCompleted ? 'rocket_launch' : 'visibility'}
                </span>
                {isCompleted ? 'Deployment' : 'Preview'}
            </h3>

            <div className="space-y-4">
                {deploymentError && (
                    <div className="p-4 rounded-lg border border-red-500/40 bg-red-500/10">
                        <p className="text-sm font-semibold text-red-300">Deployment Issue</p>
                        <p className="text-xs text-red-200 mt-1">{deploymentError}</p>
                    </div>
                )}

                {deploymentStatus && !deploymentError && (
                    <div className="p-4 rounded-lg border border-border bg-muted/40">
                        <p className="text-sm font-medium text-card-foreground">
                            Deployment status: {deploymentStatus}
                        </p>
                        {previewTarget && (
                            <p className="text-xs text-muted-foreground mt-1 break-all">
                                Runtime target: {previewTarget}
                            </p>
                        )}
                    </div>
                )}

                {/* Web Preview */}
                {previewUrl && (
                    <div className="flex items-center justify-between p-4 bg-muted/50 rounded-lg border border-border">
                        <div className="flex items-center gap-3">
                            <div className="p-2 rounded-lg bg-green-500/10">
                                <span className="material-symbols-outlined text-green-500">language</span>
                            </div>
                            <div>
                                <p className="text-sm font-medium text-card-foreground">
                                    {isCompleted ? 'Live Site' : 'Preview Environment'}
                                </p>
                                <p className="text-xs text-muted-foreground truncate max-w-[300px]">
                                    {previewUrl}
                                </p>
                            </div>
                        </div>
                        <a
                            href={previewUrl}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="flex items-center gap-1.5 px-3 py-2 bg-green-500 hover:bg-green-600 text-white text-xs font-medium rounded-lg transition-colors"
                        >
                            <span className="material-symbols-outlined text-[16px]">open_in_new</span>
                            Open
                        </a>
                    </div>
                )}

                {/* App/Artifact Downloads */}
                {downloadEntries.map((entry, index) => (
                    <div
                        key={`${entry.os}-${entry.artifactType || 'artifact'}-${index}`}
                        className="flex items-center justify-between p-4 bg-muted/50 rounded-lg border border-border"
                    >
                        <div className="flex items-center gap-3">
                            <div className="p-2 rounded-lg bg-purple-500/10">
                                <span className="material-symbols-outlined text-purple-500">
                                    download
                                </span>
                            </div>
                            <div>
                                <p className="text-sm font-medium text-card-foreground">
                                    {isCompleted ? 'Released Artifact' : 'Test Build Artifact'}
                                </p>
                                <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                                    <span className="px-1.5 py-0.5 bg-purple-500/10 text-purple-500 rounded font-semibold">
                                        {entry.label}
                                    </span>
                                    {appVersion && (
                                        <span className="px-1.5 py-0.5 bg-purple-500/10 text-purple-500 rounded font-mono">
                                            v{appVersion}
                                        </span>
                                    )}
                                    <span className="truncate max-w-[280px]">{entry.url}</span>
                                </div>
                            </div>
                        </div>
                        <a
                            href={entry.url}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="flex items-center gap-1.5 px-3 py-2 bg-purple-500 hover:bg-purple-600 text-white text-xs font-medium rounded-lg transition-colors"
                        >
                            <span className="material-symbols-outlined text-[16px]">download</span>
                            Download
                        </a>
                    </div>
                ))}

                {/* QR Code for mobile (placeholder - would need actual QR generation) */}
                {downloadEntries.length > 0 && (
                    <div className="flex items-center gap-4 p-4 bg-muted/50 rounded-lg border border-dashed border-border">
                        <div className="size-20 bg-card rounded-lg flex items-center justify-center border border-border">
                            <span className="material-symbols-outlined text-4xl text-muted-foreground">qr_code_2</span>
                        </div>
                        <div>
                            <p className="text-sm font-medium text-card-foreground mb-1">
                                Scan to Install
                            </p>
                            <p className="text-xs text-muted-foreground">
                                Scan the QR code with your device camera to install the test build directly.
                            </p>
                        </div>
                    </div>
                )}
            </div>
        </div>
    );
}
