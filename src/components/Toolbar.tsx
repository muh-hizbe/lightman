// Top toolbar — brand name + environment selector + manage environments button.
//
// Task 19.1: Environment selector dropdown highlighting the active env, plus
//            a button to open the EnvironmentEditor modal.
// Requirements: 3.6, 8.2

import React, { useEffect, useRef, useState } from "react";
import { useEnvStore } from "../stores/envStore";
import { useSettingsStore, type Theme } from "../stores/settingsStore";
import EnvironmentEditor from "./EnvironmentEditor";

interface ToolbarProps {
  sidebarOpen: boolean;
  onToggleSidebar: () => void;
}

export default function Toolbar({ sidebarOpen, onToggleSidebar }: ToolbarProps) {
  const {
    environments,
    activeEnvironmentId,
    loadEnvironments,
    setActiveEnvironment,
    loadVariables,
    loadGlobalVariables,
  } = useEnvStore();

  const { theme, setTheme } = useSettingsStore();

  const [showEditor, setShowEditor] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const settingsRef = useRef<HTMLDivElement>(null);

  // Load environments and global variables on first mount.
  useEffect(() => {
    loadEnvironments();
    loadGlobalVariables();
  }, [loadEnvironments, loadGlobalVariables]);

  // Load variables whenever the active environment changes.
  useEffect(() => {
    if (activeEnvironmentId) {
      loadVariables(activeEnvironmentId);
    }
  }, [activeEnvironmentId, loadVariables]);

  // Close settings dropdown when clicking outside.
  useEffect(() => {
    if (!showSettings) return;
    const handler = (e: MouseEvent) => {
      if (settingsRef.current && !settingsRef.current.contains(e.target as Node)) {
        setShowSettings(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [showSettings]);

  // ---------------------------------------------------------------------------
  // SVG icons — stroke-based, strokeWidth 1.5, round caps, 16×16 viewport
  // ---------------------------------------------------------------------------

  const IconSun = () => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <circle cx="8" cy="8" r="3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <path
        d="M8 1.5v1M8 13.5v1M1.5 8h1M13.5 8h1M3.4 3.4l.7.7M11.9 11.9l.7.7M11.9 3.4l-.7.7M3.4 11.9l.7-.7"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );

  const IconMoon = () => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path
        d="M13.5 9.5A6 6 0 0 1 6.5 2.5a6 6 0 1 0 7 7Z"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );

  const IconMonitor = () => (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <rect x="1" y="2" width="14" height="9" rx="1.5" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
      <path d="M5 14h6M8 11v3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  );

  const THEME_OPTIONS: { value: Theme; label: string; icon: React.ReactElement }[] = [
    { value: "light",  label: "Light",  icon: <IconSun /> },
    { value: "dark",   label: "Dark",   icon: <IconMoon /> },
    { value: "system", label: "System", icon: <IconMonitor /> },
  ];

  const handleEnvChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    if (value) {
      try {
        await setActiveEnvironment(value);
      } catch {
        // Silently ignore — the user can retry.
      }
    }
  };

  return (
    <>
      <header className="app-toolbar" role="banner">
        {/* Sidebar toggle */}
        <button
          className="btn btn--ghost btn--icon sidebar-toggle"
          title={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
          aria-label={sidebarOpen ? "Hide sidebar" : "Show sidebar"}
          aria-expanded={sidebarOpen}
          onClick={onToggleSidebar}
        >
          <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            aria-hidden="true"
          >
            {sidebarOpen ? (
              /* two columns, left one highlighted */
              <>
                <rect x="1" y="2" width="4" height="12" rx="1" fill="currentColor" opacity="0.9" />
                <rect x="7" y="2" width="8" height="12" rx="1" fill="currentColor" opacity="0.35" />
              </>
            ) : (
              /* two columns, both muted */
              <>
                <rect x="1" y="2" width="4" height="12" rx="1" fill="currentColor" opacity="0.35" />
                <rect x="7" y="2" width="8" height="12" rx="1" fill="currentColor" opacity="0.35" />
              </>
            )}
          </svg>
        </button>

        <img src="/logo.png" alt="Lightman" className="app-toolbar__logo" width="20" height="20" />
        <span className="app-toolbar__brand">Lightman</span>

        <div className="app-toolbar__spacer" />

        {/* Environment selector */}
        <div className="env-selector">
          <span className="env-selector__label">Environment:</span>
          <select
            className="env-selector__select"
            value={activeEnvironmentId ?? ""}
            onChange={handleEnvChange}
            aria-label="Active environment"
          >
            <option value="">No environment</option>
            {environments.map((env) => (
              <option key={env.id} value={env.id}>
                {env.name}
              </option>
            ))}
          </select>

          {/* Button to open environment editor modal */}
          <button
            className="btn btn--secondary btn--icon env-selector__manage-btn"
            title="Manage environments"
            aria-label="Manage environments"
            onClick={() => setShowEditor(true)}
          >
            ⚙
          </button>
        </div>

        {/* Settings menu */}
        <div className="settings-menu" ref={settingsRef}>
          <button
            className="btn btn--ghost btn--icon"
            title="Settings"
            aria-label="Settings"
            aria-expanded={showSettings}
            aria-haspopup="true"
            onClick={() => setShowSettings((v) => !v)}
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
              <path
                d="M8 10a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
              <path
                d="M8 1.5v1m0 11v1M1.5 8h1m11 0h1m-2.55-4.95-.7.7M3.75 12.25l-.7.7m0-9.9.7.7m8.5 8.5.7.7"
                stroke="currentColor"
                strokeWidth="1.5"
                strokeLinecap="round"
              />
            </svg>
          </button>

          {showSettings && (
            <div className="settings-menu__dropdown" role="menu" aria-label="Settings">
              <div className="settings-menu__header">Tema</div>
              {THEME_OPTIONS.map((opt) => (
                <button
                  key={opt.value}
                  role="menuitemradio"
                  aria-checked={theme === opt.value}
                  className={`settings-menu__item${theme === opt.value ? " settings-menu__item--active" : ""}`}
                  onClick={() => {
                    setTheme(opt.value);
                    setShowSettings(false);
                  }}
                >
                  <span className="settings-menu__item-check">
                    {theme === opt.value ? "✓" : ""}
                  </span>
                  <span>{opt.icon}</span>
                  <span>{opt.label}</span>
                </button>
              ))}
            </div>
          )}
        </div>
      </header>

      {/* Environment editor modal — rendered in the toolbar so it stays above all panels */}
      {showEditor && <EnvironmentEditor onClose={() => setShowEditor(false)} />}
    </>
  );
}
