/**
 * Shared form controls for project settings panels
 * - SettingRow: Layout wrapper for individual settings
 * - ToggleSwitch: Boolean toggle control
 * - NumberInput: Numeric input with increment/decrement
 * - TextInput: Text input field
 * - SelectInput: Dropdown select control
 */

import type { ReactNode, ChangeEvent } from 'react';

// ============================================================================
// Setting Row - Layout wrapper for individual settings
// ============================================================================

interface SettingRowProps {
    icon: string;
    iconColor: string;
    title: string;
    description: string;
    hint?: string;
    children: ReactNode;
}

export function SettingRow({ icon, iconColor, title, description, hint, children }: SettingRowProps) {
    return (
        <div className="flex items-start justify-between gap-4 p-4 bg-slate-50 dark:bg-slate-800/50 rounded-lg border border-slate-200 dark:border-slate-700">
            <div className="flex gap-3 flex-1">
                <div className="flex-shrink-0 w-10 h-10 rounded-lg bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-600 flex items-center justify-center">
                    <span className={`material-symbols-outlined ${iconColor}`}>{icon}</span>
                </div>
                <div className="flex-1 min-w-0">
                    <p className="font-medium text-slate-900 dark:text-white">{title}</p>
                    <p className="text-sm text-slate-500 dark:text-slate-400 mt-0.5">{description}</p>
                    {hint && (
                        <p className="text-xs text-slate-400 dark:text-slate-500 mt-2 flex items-center gap-1">
                            <span className="material-symbols-outlined text-[14px]">info</span>
                            {hint}
                        </p>
                    )}
                </div>
            </div>
            <div className="flex-shrink-0">{children}</div>
        </div>
    );
}

// ============================================================================
// Toggle Switch - Boolean toggle control
// ============================================================================

interface ToggleSwitchProps {
    checked: boolean;
    onChange: (checked: boolean) => void;
    disabled?: boolean;
    ariaLabel?: string;
}

export function ToggleSwitch({ checked, onChange, disabled, ariaLabel }: ToggleSwitchProps) {
    return (
        <button
            type="button"
            role="switch"
            aria-checked={checked}
            aria-label={ariaLabel}
            onClick={() => !disabled && onChange(!checked)}
            disabled={disabled}
            className={`
                relative inline-flex h-6 w-11 items-center rounded-full transition-colors
                focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2
                ${checked ? 'bg-primary' : 'bg-slate-300 dark:bg-slate-600'}
                ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
            `}
        >
            <span
                className={`
                    inline-block h-4 w-4 transform rounded-full bg-white shadow-sm transition-transform
                    ${checked ? 'translate-x-6' : 'translate-x-1'}
                `}
            />
        </button>
    );
}

// ============================================================================
// Number Input - Numeric input with increment/decrement buttons
// ============================================================================

interface NumberInputProps {
    value: number;
    onChange: (value: number) => void;
    min?: number;
    max?: number;
    suffix?: string;
    disabled?: boolean;
}

export function NumberInput({ value, onChange, min = 0, max = 100, suffix, disabled }: NumberInputProps) {
    const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
        const newValue = parseInt(e.target.value, 10);
        if (!isNaN(newValue) && newValue >= min && newValue <= max) {
            onChange(newValue);
        }
    };

    const handleIncrement = () => {
        if (value < max) onChange(value + 1);
    };

    const handleDecrement = () => {
        if (value > min) onChange(value - 1);
    };

    return (
        <div className="flex items-center gap-1">
            <button
                type="button"
                onClick={handleDecrement}
                disabled={disabled || value <= min}
                className="p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed text-slate-600 dark:text-slate-400"
            >
                <span className="material-symbols-outlined text-[18px]">remove</span>
            </button>
            <div className="relative">
                <input
                    type="number"
                    value={value}
                    onChange={handleChange}
                    min={min}
                    max={max}
                    disabled={disabled}
                    className={`
                        w-16 px-2 py-1.5 text-center text-sm font-medium rounded-lg border
                        bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-600
                        text-slate-900 dark:text-white
                        focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary
                        disabled:opacity-50 disabled:cursor-not-allowed
                        [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none
                        ${suffix ? 'pr-8' : ''}
                    `}
                />
                {suffix && (
                    <span className="absolute right-2 top-1/2 -translate-y-1/2 text-xs text-slate-400">
                        {suffix}
                    </span>
                )}
            </div>
            <button
                type="button"
                onClick={handleIncrement}
                disabled={disabled || value >= max}
                className="p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-700 disabled:opacity-50 disabled:cursor-not-allowed text-slate-600 dark:text-slate-400"
            >
                <span className="material-symbols-outlined text-[18px]">add</span>
            </button>
        </div>
    );
}

// ============================================================================
// Text Input - Simple text input field
// ============================================================================

interface TextInputProps {
    value: string;
    onChange: (value: string) => void;
    placeholder?: string;
    disabled?: boolean;
}

export function TextInput({ value, onChange, placeholder, disabled }: TextInputProps) {
    return (
        <input
            type="text"
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            className={`
                w-40 px-3 py-1.5 text-sm rounded-lg border
                bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-600
                text-slate-900 dark:text-white placeholder-slate-400
                focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary
                disabled:opacity-50 disabled:cursor-not-allowed
            `}
        />
    );
}

// ============================================================================
// Select Input - Dropdown select control
// ============================================================================

interface SelectOption {
    value: string;
    label: string;
}

interface SelectInputProps {
    value: string;
    onChange: (value: string) => void;
    options: SelectOption[];
    disabled?: boolean;
}

export function SelectInput({ value, onChange, options, disabled }: SelectInputProps) {
    return (
        <select
            value={value}
            onChange={(e) => onChange(e.target.value)}
            disabled={disabled}
            className={`
                w-40 px-3 py-1.5 text-sm rounded-lg border
                bg-white dark:bg-slate-800 border-slate-200 dark:border-slate-600
                text-slate-900 dark:text-white
                focus:outline-none focus:ring-2 focus:ring-primary focus:border-primary
                disabled:opacity-50 disabled:cursor-not-allowed
            `}
        >
            {options.map((option) => (
                <option key={option.value} value={option.value}>
                    {option.label}
                </option>
            ))}
        </select>
    );
}

// ============================================================================
// Multi-Select Chips - Multiple selection with chips display
// ============================================================================

interface MultiSelectChipsProps {
    selected: string[];
    onChange: (selected: string[]) => void;
    options: SelectOption[];
    disabled?: boolean;
}

export function MultiSelectChips({ selected, onChange, options, disabled }: MultiSelectChipsProps) {
    const toggleOption = (value: string) => {
        if (disabled) return;
        if (selected.includes(value)) {
            onChange(selected.filter((v) => v !== value));
        } else {
            onChange([...selected, value]);
        }
    };

    return (
        <div className="flex flex-wrap gap-2">
            {options.map((option) => {
                const isSelected = selected.includes(option.value);
                return (
                    <button
                        key={option.value}
                        type="button"
                        onClick={() => toggleOption(option.value)}
                        disabled={disabled}
                        className={`
                            px-3 py-1 text-xs font-medium rounded-full border transition-colors
                            ${isSelected
                                ? 'bg-primary text-primary-foreground border-primary'
                                : 'bg-white dark:bg-slate-800 text-slate-600 dark:text-slate-400 border-slate-200 dark:border-slate-600 hover:border-primary hover:text-primary'
                            }
                            ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
                        `}
                    >
                        {option.label}
                    </button>
                );
            })}
        </div>
    );
}
