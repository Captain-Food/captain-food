import type { Model, RefNode, SchemaNode, SourceFile } from './model.ts';
import { SOURCE_FILES } from './model.ts';

/** A parsed `$ref`: which file and which (possibly nested) definition path it points at. */
export interface ParsedRef {
  file: string;
  /** Path segments after `#/` (usually one: the definition name). */
  path: string[];
}

export function isRefNode(value: unknown): value is RefNode {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as Record<string, unknown>).$ref === 'string'
  );
}

/**
 * Parse a `$ref`. Two forms exist in the specs:
 *  - cross-file: `'scalars.yaml#/RestaurantId'` → `{ file: 'scalars.yaml', path: ['RestaurantId'] }`
 *  - local (same file): `'#/Address'` → `{ file: '', path: ['Address'] }` (file resolved from context)
 */
export function parseRef(ref: string): ParsedRef | null {
  const hash = ref.indexOf('#/');
  if (hash === -1) return null;
  const file = ref.slice(0, hash); // '' for a local ref
  const pointer = ref.slice(hash + 2);
  if (!pointer) return null;
  return { file, path: pointer.split('/').filter(Boolean) };
}

export function isSourceFile(file: string): file is SourceFile {
  return (SOURCE_FILES as readonly string[]).includes(file);
}

/** The file a ref points at, resolving a local (`#/...`) ref against the file it appears in. */
export function refTargetFile(ref: string, contextFile: SourceFile): SourceFile | null {
  const parsed = parseRef(ref);
  if (!parsed) return null;
  const file = parsed.file === '' ? contextFile : parsed.file;
  return isSourceFile(file) ? file : null;
}

/**
 * Resolve a ref to its target node, or null if it does not exist.
 * `contextFile` is the file the ref appears in, used to resolve local (`#/...`) refs.
 */
export function resolveRef(model: Model, ref: string, contextFile: SourceFile): SchemaNode | null {
  const parsed = parseRef(ref);
  if (!parsed) return null;
  const file = parsed.file === '' ? contextFile : parsed.file;
  if (!isSourceFile(file)) return null;
  const [head, ...rest] = parsed.path;
  if (head === undefined) return null;
  let node: unknown = model.defs[file][head];
  for (const segment of rest) {
    if (typeof node !== 'object' || node === null) return null;
    node = (node as Record<string, unknown>)[segment];
  }
  return node === undefined || node === null ? null : (node as SchemaNode);
}

/** The bare definition name a ref targets (its first path segment). */
export function refName(ref: string): string | null {
  return parseRef(ref)?.path[0] ?? null;
}

/** One `$ref` occurrence found while walking the tree, with a human-readable location. */
export interface RefOccurrence {
  ref: string;
  /** Dotted location for diagnostics, e.g. `events.yaml/RestaurantRegistered.properties.address`. */
  location: string;
}

/** Deep-walk any value, collecting every `$ref` string with its location. */
export function collectRefs(root: unknown, baseLocation: string): RefOccurrence[] {
  const out: RefOccurrence[] = [];
  const walk = (value: unknown, location: string): void => {
    if (Array.isArray(value)) {
      value.forEach((item, i) => walk(item, `${location}[${i}]`));
      return;
    }
    if (typeof value === 'object' && value !== null) {
      if (isRefNode(value)) {
        out.push({ ref: value.$ref, location });
        return;
      }
      for (const [key, child] of Object.entries(value)) {
        walk(child, `${location}.${key}`);
      }
    }
  };
  walk(root, baseLocation);
  return out;
}
