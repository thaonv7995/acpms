/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  darkMode: 'class',
  theme: {
    extend: {
      // New HSL-based color system (Vibe-Kanban style)
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        success: {
          DEFAULT: "hsl(var(--success))",
          foreground: "hsl(var(--success-foreground))",
        },
        warning: {
          DEFAULT: "hsl(var(--warning))",
          foreground: "hsl(var(--warning-foreground))",
        },
        info: {
          DEFAULT: "hsl(var(--info))",
          foreground: "hsl(var(--info-foreground))",
        },
        neutral: {
          DEFAULT: "hsl(var(--neutral))",
          foreground: "hsl(var(--neutral-foreground))",
        },

        // Legacy colors (kept for backward compatibility - will be phased out)
        'primary-legacy': '#0d7ff2',
        'background-light': '#f5f7f8',
        'background-dark': '#101922',
        'surface-dark': '#1c2630',
        'surface-light': '#ffffff',
        'surface-border': '#283039',
        'border-dark': '#2d3b4a',
        'terminal-bg': '#0f1216',
      },

      // Custom fontSize scale (downshifted by 1 from Tailwind defaults)
      fontSize: {
        xs: ['0.625rem', { lineHeight: '0.875rem' }],    // 10px/14px
        sm: ['0.75rem', { lineHeight: '1rem' }],         // 12px/16px
        base: ['0.875rem', { lineHeight: '1.25rem' }],   // 14px/20px ← Key change
        lg: ['1rem', { lineHeight: '1.5rem' }],          // 16px/24px
        xl: ['1.125rem', { lineHeight: '1.75rem' }],     // 18px/28px
        '2xl': ['1.25rem', { lineHeight: '1.875rem' }],  // 20px/30px
        '3xl': ['1.5rem', { lineHeight: '2rem' }],       // 24px/32px
      },

      // Font families
      fontFamily: {
        'chivo-mono': ['Chivo Mono', 'Noto Emoji', 'monospace'],
        // Keep legacy fonts for gradual migration
        display: ['Space Grotesk', 'sans-serif'],
        body: ['Noto Sans', 'sans-serif'],
        mono: ['JetBrains Mono', 'monospace'],
      },

      // Border radius using CSS variable
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },

      // Diagonal lines background pattern
      backgroundImage: {
        'diagonal-lines': `
          repeating-linear-gradient(-45deg, hsl(var(--border) / 0.4) 0 2px, transparent 1px 12px),
          linear-gradient(hsl(var(--background)), hsl(var(--background)))
        `,
      },
    },
  },
  plugins: [],
}
