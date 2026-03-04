import { Component, ErrorInfo, ReactNode } from 'react';
import { logger } from '@/lib/logger';

interface Props {
    children?: ReactNode;
    fallback?: ReactNode;
}

interface State {
    hasError: boolean;
    error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false
    };

    public static getDerivedStateFromError(error: Error): State {
        // Update state so the next render will show the fallback UI.
        return { hasError: true, error };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        logger.error('Uncaught error:', error, errorInfo);
    }

    public render() {
        if (this.state.hasError) {
            if (this.props.fallback) {
                return this.props.fallback;
            }
            return (
                <div className="p-6 m-4 bg-destructive/10 border border-destructive rounded-lg flex flex-col items-center justify-center min-h-[300px]">
                    <span className="material-symbols-outlined text-4xl text-destructive mb-3">error</span>
                    <h2 className="text-destructive font-bold text-xl mb-2">Something went wrong</h2>
                    <p className="text-muted-foreground mb-4">The component failed to render properly.</p>
                    {this.state.error && (
                        <pre className="bg-background text-foreground/80 p-4 rounded-md text-xs sm:text-sm overflow-auto max-w-full">
                            {this.state.error.message}
                        </pre>
                    )}
                    <button
                        className="mt-4 px-4 py-2 bg-destructive text-destructive-foreground hover:bg-destructive/90 rounded-md text-sm font-medium transition-colors"
                        onClick={() => this.setState({ hasError: false })}
                    >
                        Try again
                    </button>
                </div>
            );
        }

        return this.props.children;
    }
}
