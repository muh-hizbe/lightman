// Zustand store for Environments and Variables state.
//
// Requirements: 3.1, 3.3, 3.6, 8.2

import { create } from "zustand";
import type { Environment, Variable } from "../types";
import * as api from "../api/environments";

interface EnvState {
  environments: Environment[];
  /** Variables per environment ID (loaded on demand). */
  variablesByEnv: Record<string, Variable[]>;
  /** Global variables (environment_id === null). */
  globalVariables: Variable[];
  /** ID of the currently active environment, or null. */
  activeEnvironmentId: string | null;
  loading: boolean;
  error: string | null;

  // ---------------------------------------------------------------------------
  // Actions — Environments
  // ---------------------------------------------------------------------------

  loadEnvironments: () => Promise<void>;
  createEnvironment: (name: string) => Promise<Environment>;
  updateEnvironment: (id: string, name: string) => Promise<void>;
  deleteEnvironment: (id: string) => Promise<void>;
  setActiveEnvironment: (id: string) => Promise<void>;
  clearActiveEnvironment: () => Promise<void>;

  // ---------------------------------------------------------------------------
  // Actions — Variables
  // ---------------------------------------------------------------------------

  loadVariables: (environmentId: string) => Promise<void>;
  loadGlobalVariables: () => Promise<void>;
  createVariable: (
    environmentId: string | null,
    key: string,
    value: string,
    enabled: boolean
  ) => Promise<Variable>;
  updateVariable: (
    id: string,
    key: string,
    value: string,
    enabled: boolean
  ) => Promise<void>;
  deleteVariable: (id: string, environmentId: string | null) => Promise<void>;
}

export const useEnvStore = create<EnvState>((set, _get) => ({
  environments: [],
  variablesByEnv: {},
  globalVariables: [],
  activeEnvironmentId: null,
  loading: false,
  error: null,

  // ---------------------------------------------------------------------------
  // Environments
  // ---------------------------------------------------------------------------

  loadEnvironments: async () => {
    set({ loading: true, error: null });
    try {
      const environments = await api.listEnvironments();
      const active = environments.find((e) => e.is_active) ?? null;
      set({
        environments,
        activeEnvironmentId: active?.id ?? null,
        loading: false,
      });
    } catch (err) {
      set({ error: String(err), loading: false });
    }
  },

  createEnvironment: async (name) => {
    const env = await api.createEnvironment(name);
    set((s) => ({ environments: [...s.environments, env] }));
    return env;
  },

  updateEnvironment: async (id, name) => {
    const updated = await api.updateEnvironment(id, name);
    set((s) => ({
      environments: s.environments.map((e) => (e.id === id ? updated : e)),
    }));
  },

  deleteEnvironment: async (id) => {
    await api.deleteEnvironment(id);
    set((s) => {
      const envs = s.environments.filter((e) => e.id !== id);
      const varMap = { ...s.variablesByEnv };
      delete varMap[id];
      const activeId =
        s.activeEnvironmentId === id
          ? (envs.find((e) => e.is_active)?.id ?? null)
          : s.activeEnvironmentId;
      return {
        environments: envs,
        variablesByEnv: varMap,
        activeEnvironmentId: activeId,
      };
    });
  },

  setActiveEnvironment: async (id) => {
    const updated = await api.setActiveEnvironment(id);
    set((s) => ({
      environments: s.environments.map((e) =>
        e.id === id ? updated : { ...e, is_active: false }
      ),
      activeEnvironmentId: id,
    }));
  },

  clearActiveEnvironment: async () => {
    await api.clearActiveEnvironment();
    set((s) => ({
      environments: s.environments.map((e) => ({ ...e, is_active: false })),
      activeEnvironmentId: null,
    }));
  },

  // ---------------------------------------------------------------------------
  // Variables
  // ---------------------------------------------------------------------------

  loadVariables: async (environmentId) => {
    try {
      const vars = await api.listVariables(environmentId);
      set((s) => ({
        variablesByEnv: { ...s.variablesByEnv, [environmentId]: vars },
      }));
    } catch (err) {
      set({ error: String(err) });
    }
  },

  loadGlobalVariables: async () => {
    try {
      const vars = await api.listGlobalVariables();
      set({ globalVariables: vars });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  createVariable: async (environmentId, key, value, enabled) => {
    const variable = await api.createVariable(environmentId, key, value, enabled);
    set((s) => {
      if (environmentId === null) {
        return { globalVariables: [...s.globalVariables, variable] };
      }
      const existing = s.variablesByEnv[environmentId] ?? [];
      return {
        variablesByEnv: {
          ...s.variablesByEnv,
          [environmentId]: [...existing, variable],
        },
      };
    });
    return variable;
  },

  updateVariable: async (id, key, value, enabled) => {
    const updated = await api.updateVariable(id, key, value, enabled);
    set((s) => {
      const envId = updated.environment_id;
      if (envId === null) {
        return {
          globalVariables: s.globalVariables.map((v) =>
            v.id === id ? updated : v
          ),
        };
      }
      const existing = s.variablesByEnv[envId] ?? [];
      return {
        variablesByEnv: {
          ...s.variablesByEnv,
          [envId]: existing.map((v) => (v.id === id ? updated : v)),
        },
      };
    });
  },

  deleteVariable: async (id, environmentId) => {
    await api.deleteVariable(id);
    set((s) => {
      if (environmentId === null) {
        return {
          globalVariables: s.globalVariables.filter((v) => v.id !== id),
        };
      }
      const existing = s.variablesByEnv[environmentId] ?? [];
      return {
        variablesByEnv: {
          ...s.variablesByEnv,
          [environmentId]: existing.filter((v) => v.id !== id),
        },
      };
    });
  },
}));
