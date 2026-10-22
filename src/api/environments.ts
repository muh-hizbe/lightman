// Tauri invoke wrappers for environment and variable commands.
// Mirrors commands/environment.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type { Environment, Variable } from "../types";

// ---------------------------------------------------------------------------
// Environment API
// ---------------------------------------------------------------------------

export function createEnvironment(name: string): Promise<Environment> {
  return invoke("create_environment", { name });
}

export function listEnvironments(): Promise<Environment[]> {
  return invoke("list_environments");
}

export function updateEnvironment(id: string, name: string): Promise<Environment> {
  return invoke("update_environment", { id, name });
}

export function deleteEnvironment(id: string): Promise<void> {
  return invoke("delete_environment", { id });
}

export function setActiveEnvironment(id: string): Promise<Environment> {
  return invoke("set_active_environment", { id });
}

// ---------------------------------------------------------------------------
// Variable API
// ---------------------------------------------------------------------------

export function createVariable(
  environmentId: string | null,
  key: string,
  value: string,
  enabled: boolean
): Promise<Variable> {
  return invoke("create_variable", {
    environmentId,
    key,
    value,
    enabled,
  });
}

export function listVariables(environmentId: string): Promise<Variable[]> {
  return invoke("list_variables", { environmentId });
}

export function listGlobalVariables(): Promise<Variable[]> {
  return invoke("list_global_variables");
}

export function updateVariable(
  id: string,
  key: string,
  value: string,
  enabled: boolean
): Promise<Variable> {
  return invoke("update_variable", { id, key, value, enabled });
}

export function deleteVariable(id: string): Promise<void> {
  return invoke("delete_variable", { id });
}
