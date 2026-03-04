import { useState, FormEvent } from 'react';
import { useNavigate } from 'react-router-dom';
import { login } from '../api/auth';
import { ApiError } from '../api/client';

export function LoginPage() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError('');
    setLoading(true);

    try {
      await login({ email, password });
      navigate('/dashboard');
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.message);
      } else {
        setError('An unexpected error occurred');
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen w-full bg-[#0d1117] text-white font-display flex flex-col relative overflow-hidden">
      {/* Grid Background Pattern */}
      <div className="absolute inset-0 z-0 opacity-[0.03] pointer-events-none"
        style={{
          backgroundImage: `linear-gradient(#fff 1px, transparent 1px), linear-gradient(90deg, #fff 1px, transparent 1px)`,
          backgroundSize: '40px 40px'
        }}>
      </div>

      {/* Radial Gradient Glow */}
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[800px] h-[800px] bg-primary/5 rounded-full blur-[120px] pointer-events-none z-0"></div>

      {/* Header */}
      <header className="relative z-10 w-full px-6 py-6 flex justify-center items-center max-w-[1600px] mx-auto">
        <div className="flex items-center gap-3">
          <div className="size-8 rounded bg-primary flex items-center justify-center text-white shrink-0 shadow-lg shadow-primary/25">
            <span className="material-symbols-outlined text-xl">deployed_code</span>
          </div>
          <h1 className="text-xl font-bold tracking-tight">ACPMS</h1>
        </div>
      </header>

      {/* Main Content */}
      <main className="relative z-10 flex-1 flex flex-col items-center justify-center p-4">
        <div className="w-full max-w-[420px]">
          <div className="bg-[#161b22] border border-[rgba(240,246,252,0.1)] rounded-2xl p-6 sm:p-8 shadow-2xl relative overflow-hidden">
            {/* Top Highlight Line */}
            <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-transparent via-primary/50 to-transparent opacity-50"></div>

            <div className="text-center mb-8">
              <h2 className="text-2xl font-bold mb-2">Welcome Back</h2>
              <p className="text-slate-400 text-sm">Internal access only. Please sign in to continue.</p>
            </div>

            <form onSubmit={handleSubmit} className="flex flex-col gap-5">
              <div className="space-y-1.5">
                <label className="text-xs font-bold text-slate-300 uppercase tracking-wide" htmlFor="email">Email or Username</label>
                <div className="relative group">
                  <input
                    id="email"
                    type="email"
                    value={email}
                    onChange={(e) => setEmail(e.target.value)}
                    required
                    className="w-full bg-[#0d1117] border border-[rgba(240,246,252,0.1)] text-white text-sm rounded-lg focus:ring-1 focus:ring-primary focus:border-primary block p-3 pr-10 transition-all group-hover:border-[rgba(240,246,252,0.2)] placeholder-slate-600"
                    placeholder="name@example.com"
                  />
                  <span className="absolute inset-y-0 right-0 flex items-center pr-3 text-slate-500 pointer-events-none">
                    <span className="material-symbols-outlined text-[20px]">mail</span>
                  </span>
                </div>
              </div>

              <div className="space-y-1.5">
                <label className="text-xs font-bold text-slate-300 uppercase tracking-wide" htmlFor="password">Password</label>
                <div className="relative group">
                  <input
                    id="password"
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    required
                    className="w-full bg-[#0d1117] border border-[rgba(240,246,252,0.1)] text-white text-sm rounded-lg focus:ring-1 focus:ring-primary focus:border-primary block p-3 pr-12 transition-all group-hover:border-[rgba(240,246,252,0.2)] placeholder-slate-600"
                    placeholder="••••••••"
                  />
                  <button
                    type="button"
                    onClick={() => setShowPassword((p) => !p)}
                    className="absolute inset-y-0 right-0 flex items-center pr-3 text-slate-500 hover:text-slate-300 transition-colors"
                    tabIndex={-1}
                    aria-label={showPassword ? 'Hide password' : 'Show password'}
                  >
                    <span className="material-symbols-outlined text-[20px]">
                      {showPassword ? 'visibility_off' : 'visibility'}
                    </span>
                  </button>
                </div>
              </div>

              {error && (
                <div className="text-red-400 text-sm bg-red-900/20 border border-red-800 rounded-lg p-3">
                  {error}
                </div>
              )}

              <button
                type="submit"
                disabled={loading}
                className="w-full bg-primary hover:bg-primary/90 text-primary-foreground font-bold py-3 px-4 rounded-lg transition-all shadow-[0_0_20px_rgba(13,127,242,0.3)] hover:shadow-[0_0_25px_rgba(13,127,242,0.5)] active:scale-[0.98] mt-2 disabled:opacity-50"
              >
                {loading ? 'Logging in...' : 'Log In'}
              </button>
            </form>

            <div className="mt-8 text-center">
              <p className="text-xs text-slate-500">
                Protected by enterprise-grade encryption. <a href="#" className="underline hover:text-slate-400">Privacy Policy</a>
              </p>
            </div>
          </div>
        </div>
      </main>
    </div>
  );
}
