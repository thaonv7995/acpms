import { ReactNode } from 'react';
import { Navigate } from 'react-router-dom';
import { isAuthenticated, isSystemAdmin } from '../../api/auth';

interface ProtectedRouteProps {
  children: ReactNode;
  requireAdmin?: boolean;
}

export function ProtectedRoute({ children, requireAdmin = false }: ProtectedRouteProps) {
  if (!isAuthenticated()) {
    return <Navigate to="/login" replace />;
  }

  if (requireAdmin && !isSystemAdmin()) {
    return <Navigate to="/dashboard" replace />;
  }

  return <>{children}</>;
}
