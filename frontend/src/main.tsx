import ReactDOM from 'react-dom/client'
import { QueryClientProvider } from '@tanstack/react-query'
import App from './App.tsx'
import { queryClient } from './lib/queryClient'
import { silenceConsoleInProduction } from './lib/logger'
import './index.css'

silenceConsoleInProduction()

ReactDOM.createRoot(document.getElementById('root')!).render(
  // Note: StrictMode disabled to prevent double API calls in development
  // Re-enable for production-ready code to detect side effects
  // <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  // </React.StrictMode>,
)
