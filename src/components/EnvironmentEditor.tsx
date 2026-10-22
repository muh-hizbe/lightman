// EnvironmentEditor — modal panel for managing environments and variables.
//
// Task 19.2: CRUD environments + variable table (key/value/enabled) + Global Variables section.
// Requirements: 3.1, 3.3

import { useEffect, useState, useCallback, useRef } from "react";
import { useEnvStore } from "../stores/envStore";
import ConfirmDialog from "./ConfirmDialog";
import type { Environment, Variable } from "../types";

// ---------------------------------------------------------------------------
// SVG icons — stroke-based, consistent with Sidebar icons
// strokeWidth 1.5, round caps & joins, 16×16 viewport
// ---------------------------------------------------------------------------

const IconPencil = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M11.5 2.5a1.414 1.414 0 0 1 2 2L5 13H3v-2L11.5 2.5Z"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const IconTrash = () => (
  <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
    <path
      d="M2 4h12M5 4V2h6v2M6 7v5M10 7v5M3 4l1 9a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1l1-9"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface EnvironmentEditorProps {
  onClose: () => void;
}

// ---------------------------------------------------------------------------
// Small helper — row for editing a single variable
// ---------------------------------------------------------------------------

interface VarRowProps {
  variable: Variable;
  onUpdate: (id: string, key: string, value: string, enabled: boolean) => void;
  onDelete: (id: string, envId: string | null) => void;
}

function VarRow({ variable, onUpdate, onDelete }: VarRowProps) {
  const [key, setKey] = useState(variable.key);
  const [value, setValue] = useState(variable.value);
  const [enabled, setEnabled] = useState(variable.enabled);

  // Commit changes on blur
  const commit = () => {
    if (key !== variable.key || value !== variable.value || enabled !== variable.enabled) {
      onUpdate(variable.id, key, value, enabled);
    }
  };

  return (
    <tr className="var-table__row" data-disabled={!enabled ? "true" : undefined}>
      <td className="var-table__cell-enable">
        <input
          type="checkbox"
          checked={enabled}
          aria-label={`Enable variable ${variable.key || "(empty)"}`}
          onChange={(e) => {
            setEnabled(e.target.checked);
            onUpdate(variable.id, key, value, e.target.checked);
          }}
        />
      </td>
      <td className="var-table__cell-key">
        <input
          className="input var-table__input"
          type="text"
          value={key}
          placeholder="Key"
          aria-label="Variable key"
          onChange={(e) => setKey(e.target.value)}
          onBlur={commit}
        />
      </td>
      <td className="var-table__cell-value">
        <input
          className="input var-table__input"
          type="text"
          value={value}
          placeholder="Value"
          aria-label="Variable value"
          onChange={(e) => setValue(e.target.value)}
          onBlur={commit}
        />
      </td>
      <td className="var-table__cell-action">
        <button
          className="btn btn--ghost btn--icon"
          title="Delete variable"
          aria-label={`Delete variable ${variable.key || "(empty)"}`}
          onClick={() => onDelete(variable.id, variable.environment_id)}
        >
          ✕
        </button>
      </td>
    </tr>
  );
}

// ---------------------------------------------------------------------------
// New-variable row (always visible at the bottom of the table)
// ---------------------------------------------------------------------------

interface NewVarRowProps {
  onAdd: (key: string, value: string) => void;
}

function NewVarRow({ onAdd }: NewVarRowProps) {
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");

  const handleAdd = () => {
    if (!key.trim()) return;
    onAdd(key.trim(), value);
    setKey("");
    setValue("");
  };

  return (
    <tr className="var-table__row var-table__row--new">
      <td className="var-table__cell-enable" />
      <td className="var-table__cell-key">
        <input
          className="input var-table__input"
          type="text"
          value={key}
          placeholder="New key…"
          aria-label="New variable key"
          onChange={(e) => setKey(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleAdd();
          }}
        />
      </td>
      <td className="var-table__cell-value">
        <input
          className="input var-table__input"
          type="text"
          value={value}
          placeholder="Value"
          aria-label="New variable value"
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") handleAdd();
          }}
        />
      </td>
      <td className="var-table__cell-action">
        <button
          className="btn btn--primary btn--icon"
          title="Add variable"
          aria-label="Add variable"
          onClick={handleAdd}
          disabled={!key.trim()}
        >
          +
        </button>
      </td>
    </tr>
  );
}

// ---------------------------------------------------------------------------
// Variable table component
// ---------------------------------------------------------------------------

interface VarTableProps {
  variables: Variable[];
  environmentId: string | null;
}

function VarTable({ variables, environmentId }: VarTableProps) {
  const { createVariable, updateVariable, deleteVariable } = useEnvStore();

  const handleAdd = async (key: string, value: string) => {
    try {
      await createVariable(environmentId, key, value, true);
    } catch {
      // ignore — store will surface error
    }
  };

  const handleUpdate = async (id: string, key: string, value: string, enabled: boolean) => {
    try {
      await updateVariable(id, key, value, enabled);
    } catch {
      // ignore
    }
  };

  const handleDelete = async (id: string, envId: string | null) => {
    try {
      await deleteVariable(id, envId);
    } catch {
      // ignore
    }
  };

  return (
    <div className="var-table-wrapper">
      <table className="var-table" aria-label={environmentId ? "Environment variables" : "Global variables"}>
        <thead>
          <tr>
            <th className="var-table__th var-table__cell-enable">On</th>
            <th className="var-table__th var-table__cell-key">Key</th>
            <th className="var-table__th var-table__cell-value">Value</th>
            <th className="var-table__th var-table__cell-action" />
          </tr>
        </thead>
        <tbody>
          {variables.map((v) => (
            <VarRow
              key={v.id}
              variable={v}
              onUpdate={handleUpdate}
              onDelete={handleDelete}
            />
          ))}
          <NewVarRow onAdd={handleAdd} />
        </tbody>
      </table>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main EnvironmentEditor component
// ---------------------------------------------------------------------------

export default function EnvironmentEditor({ onClose }: EnvironmentEditorProps) {
  const {
    environments,
    variablesByEnv,
    globalVariables,
    createEnvironment,
    updateEnvironment,
    deleteEnvironment,
    loadVariables,
    loadGlobalVariables,
  } = useEnvStore();

  /** Which env is selected in the left panel */
  const [selectedEnvId, setSelectedEnvId] = useState<string | "global">("global");

  /** Inline-rename tracking */
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const renameInputRef = useRef<HTMLInputElement>(null);

  /** Creating new environment */
  const [isCreating, setIsCreating] = useState(false);
  const [newEnvName, setNewEnvName] = useState("");
  const newEnvInputRef = useRef<HTMLInputElement>(null);

  /** Pending delete confirmation */
  const [pendingDeleteEnvId, setPendingDeleteEnvId] = useState<string | null>(null);

  // Load global variables on mount.
  useEffect(() => {
    loadGlobalVariables();
  }, [loadGlobalVariables]);

  // Load variables when a specific env is selected.
  useEffect(() => {
    if (selectedEnvId !== "global" && !variablesByEnv[selectedEnvId]) {
      loadVariables(selectedEnvId);
    }
  }, [selectedEnvId, variablesByEnv, loadVariables]);

  // Focus rename input when renaming starts.
  useEffect(() => {
    if (renamingId !== null) {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    }
  }, [renamingId]);

  // Focus new env input.
  useEffect(() => {
    if (isCreating) {
      newEnvInputRef.current?.focus();
    }
  }, [isCreating]);

  // Close on Escape key.
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    },
    [onClose]
  );

  useEffect(() => {
    document.addEventListener("keydown", handleKeyDown);
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [handleKeyDown]);

  // ---------------------------------------------------------------------------
  // Environment CRUD handlers
  // ---------------------------------------------------------------------------

  const handleCreateEnv = async () => {
    const name = newEnvName.trim();
    if (!name) return;
    try {
      const env = await createEnvironment(name);
      setSelectedEnvId(env.id);
    } catch {
      // ignore
    }
    setIsCreating(false);
    setNewEnvName("");
  };

  const handleStartRename = (env: Environment) => {
    setRenamingId(env.id);
    setRenameValue(env.name);
  };

  const handleCommitRename = async (id: string) => {
    const name = renameValue.trim();
    if (name) {
      try {
        await updateEnvironment(id, name);
      } catch {
        // ignore
      }
    }
    setRenamingId(null);
    setRenameValue("");
  };

  const handleDeleteEnv = (id: string) => {
    setPendingDeleteEnvId(id);
  };

  const confirmDeleteEnv = async () => {
    const id = pendingDeleteEnvId;
    setPendingDeleteEnvId(null);
    if (!id) return;
    try {
      await deleteEnvironment(id);
      if (selectedEnvId === id) setSelectedEnvId("global");
    } catch {
      // ignore
    }
  };

  // ---------------------------------------------------------------------------
  // Derived state
  // ---------------------------------------------------------------------------

  const selectedEnv =
    selectedEnvId === "global"
      ? null
      : environments.find((e) => e.id === selectedEnvId) ?? null;

  const activeVariables =
    selectedEnvId === "global"
      ? globalVariables
      : (variablesByEnv[selectedEnvId] ?? []);

  // ---------------------------------------------------------------------------
  // Render
  // ---------------------------------------------------------------------------

  return (
    /* Backdrop */
    <div
      className="env-editor-backdrop"
      role="dialog"
      aria-modal="true"
      aria-label="Environment Editor"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="env-editor">
        {/* Header */}
        <div className="env-editor__header">
          <h2 className="env-editor__title">Environments</h2>
          <button
            className="btn btn--ghost btn--icon"
            onClick={onClose}
            aria-label="Close environment editor"
          >
            ✕
          </button>
        </div>

        <div className="env-editor__body">
          {/* Left pane — environment list */}
          <aside className="env-editor__sidebar">
            <div className="env-editor__sidebar-header">
              <span className="env-editor__sidebar-title">Environments</span>
              <button
                className="btn btn--ghost btn--icon"
                title="New environment"
                aria-label="Create new environment"
                onClick={() => {
                  setIsCreating(true);
                  setNewEnvName("");
                }}
              >
                +
              </button>
            </div>

            <ul className="env-list" role="listbox" aria-label="Environment list">
              {/* Global variables entry */}
              <li
                className={`env-list__item${selectedEnvId === "global" ? " env-list__item--active" : ""}`}
                role="option"
                aria-selected={selectedEnvId === "global"}
                tabIndex={0}
                onClick={() => setSelectedEnvId("global")}
                onKeyDown={(e) => {
                  if (e.key === "Enter" || e.key === " ") setSelectedEnvId("global");
                }}
              >
                <span className="env-list__icon">🌐</span>
                <span className="env-list__name">Global Variables</span>
              </li>

              {/* User environments */}
              {environments.map((env) => (
                <li
                  key={env.id}
                  className={`env-list__item${selectedEnvId === env.id ? " env-list__item--active" : ""}`}
                  role="option"
                  aria-selected={selectedEnvId === env.id}
                  tabIndex={0}
                  onClick={() => setSelectedEnvId(env.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") setSelectedEnvId(env.id);
                  }}
                >
                  {renamingId === env.id ? (
                    <input
                      ref={renameInputRef}
                      className="input env-list__rename-input"
                      type="text"
                      value={renameValue}
                      aria-label="Rename environment"
                      onChange={(e) => setRenameValue(e.target.value)}
                      onBlur={() => handleCommitRename(env.id)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleCommitRename(env.id);
                        if (e.key === "Escape") setRenamingId(null);
                        e.stopPropagation();
                      }}
                      onClick={(e) => e.stopPropagation()}
                    />
                  ) : (
                    <>
                      <span className="env-list__icon">{env.is_active ? "✅" : "○"}</span>
                      <span className="env-list__name">{env.name}</span>
                      {selectedEnvId === env.id && (
                        <span className="env-list__actions">
                          <button
                            className="btn btn--ghost btn--icon"
                            title="Rename"
                            aria-label={`Rename ${env.name}`}
                            onClick={(e) => {
                              e.stopPropagation();
                              handleStartRename(env);
                            }}
                          >
                            <IconPencil />
                          </button>
                          <button
                            className="btn btn--ghost btn--icon env-list__delete-btn"
                            title="Delete"
                            aria-label={`Delete ${env.name}`}
                            onClick={(e) => {
                              e.stopPropagation();
                              handleDeleteEnv(env.id);
                            }}
                          >
                            <IconTrash />
                          </button>
                        </span>
                      )}
                    </>
                  )}
                </li>
              ))}

              {/* New environment creation row */}
              {isCreating && (
                <li className="env-list__item env-list__item--creating">
                  <input
                    ref={newEnvInputRef}
                    className="input env-list__rename-input"
                    type="text"
                    value={newEnvName}
                    placeholder="Environment name…"
                    aria-label="New environment name"
                    onChange={(e) => setNewEnvName(e.target.value)}
                    onBlur={handleCreateEnv}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") handleCreateEnv();
                      if (e.key === "Escape") {
                        setIsCreating(false);
                        setNewEnvName("");
                      }
                    }}
                  />
                </li>
              )}
            </ul>
          </aside>

          {/* Right pane — variable table */}
          <section className="env-editor__content">
            <div className="env-editor__content-header">
              <h3 className="env-editor__content-title">
                {selectedEnvId === "global"
                  ? "Global Variables"
                  : selectedEnv?.name ?? "Variables"}
              </h3>
              {selectedEnvId !== "global" && selectedEnv && (
                <span className="env-editor__env-hint">
                  {selectedEnv.is_active ? "● Active" : "○ Inactive"}
                </span>
              )}
            </div>

            <VarTable
              variables={activeVariables}
              environmentId={selectedEnvId === "global" ? null : selectedEnvId}
            />
          </section>
        </div>
      </div>

      {/* Delete confirmation dialog */}
      {pendingDeleteEnvId && (
        <ConfirmDialog
          message="Delete this environment? All its variables will be removed."
          title="Delete Environment"
          onConfirm={confirmDeleteEnv}
          onCancel={() => setPendingDeleteEnvId(null)}
        />
      )}
    </div>
  );
}
