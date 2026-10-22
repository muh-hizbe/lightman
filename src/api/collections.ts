// Tauri invoke wrappers for collection and request commands.
// Mirrors commands/collection.rs
//
// Requirements: 8.3

import { invoke } from "@tauri-apps/api/core";
import type {
  AuthConfig,
  Collection,
  Header,
  HttpMethod,
  QueryParam,
  Request,
  RequestBody,
} from "../types";

// ---------------------------------------------------------------------------
// Collection API
// ---------------------------------------------------------------------------

export function createCollection(
  name: string,
  description?: string | null,
  parentId?: string | null,
  auth?: AuthConfig | null
): Promise<Collection> {
  return invoke("create_collection", {
    name,
    description: description ?? null,
    parentId: parentId ?? null,
    auth: auth ?? null,
  });
}

export function getCollection(id: string): Promise<Collection | null> {
  return invoke("get_collection", { id });
}

export function listCollections(): Promise<Collection[]> {
  return invoke("list_collections");
}

export function updateCollection(
  id: string,
  name: string,
  description?: string | null,
  auth?: AuthConfig | null
): Promise<Collection> {
  return invoke("update_collection", {
    id,
    name,
    description: description ?? null,
    auth: auth ?? null,
  });
}

export function deleteCollection(id: string): Promise<void> {
  return invoke("delete_collection", { id });
}

// ---------------------------------------------------------------------------
// Request API
// ---------------------------------------------------------------------------

export function createRequest(
  collectionId: string,
  name: string,
  method: HttpMethod,
  url: string,
  headers: Header[],
  params: QueryParam[],
  body: RequestBody,
  auth: AuthConfig
): Promise<Request> {
  return invoke("create_request", {
    collectionId,
    name,
    method,
    url,
    headers,
    params,
    body,
    auth,
  });
}

export function getRequest(id: string): Promise<Request | null> {
  return invoke("get_request", { id });
}

export function listRequests(collectionId: string): Promise<Request[]> {
  return invoke("list_requests", { collectionId });
}

export function updateRequest(
  id: string,
  name: string,
  method: HttpMethod,
  url: string,
  headers: Header[],
  params: QueryParam[],
  body: RequestBody,
  auth: AuthConfig
): Promise<Request> {
  return invoke("update_request", {
    id,
    name,
    method,
    url,
    headers,
    params,
    body,
    auth,
  });
}

export function deleteRequest(id: string): Promise<void> {
  return invoke("delete_request", { id });
}

export function duplicateRequest(id: string): Promise<Request> {
  return invoke("duplicate_request", { id });
}

export function reorderRequest(id: string, targetPosition: number): Promise<void> {
  return invoke("reorder_request", { id, targetPosition });
}

export function moveRequest(id: string, targetCollectionId: string): Promise<Request> {
  return invoke("move_request", { id, targetCollectionId });
}
