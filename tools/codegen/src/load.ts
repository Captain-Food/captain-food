import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { parse as parseYaml } from 'yaml';
import type {
  Actor, Api, ApiField, ApiMutation, ApiQuery, ApiType, Model, Persona, ReceiveEntry, SchemaNode,
  SourceFile, StoryActivity, StoryStep, View, ViewColumn,
} from './model.ts';
import { SOURCE_FILES } from './model.ts';

/** Top-level keys that are file metadata, not definitions. */
const META_KEYS = new Set(['version', 'description']);

function loadFile(specsDir: string, file: SourceFile): {
  defs: Record<string, SchemaNode>;
  meta: { version?: number; description?: string };
} {
  const raw = parseYaml(readFileSync(join(specsDir, file), 'utf8')) as Record<string, unknown>;
  const defs: Record<string, SchemaNode> = {};
  const meta: { version?: number; description?: string } = {};
  for (const [key, value] of Object.entries(raw ?? {})) {
    if (key === 'version' && typeof value === 'number') meta.version = value;
    else if (key === 'description' && typeof value === 'string') meta.description = value;
    else if (!META_KEYS.has(key)) defs[key] = value as SchemaNode;
  }
  return { defs, meta };
}

function toRefList(value: unknown): { $ref: string }[] {
  if (!Array.isArray(value)) return [];
  return value.filter(
    (item): item is { $ref: string } =>
      typeof item === 'object' &&
      item !== null &&
      typeof (item as Record<string, unknown>).$ref === 'string',
  );
}

function parseActors(defs: Record<string, SchemaNode>): Actor[] {
  const actors: Actor[] = [];
  for (const [name, node] of Object.entries(defs)) {
    const type = node.type;
    if (type !== 'aggregate' && type !== 'process-manager') continue;
    const rawReceives = Array.isArray(node.receives) ? node.receives : [];
    const receives: ReceiveEntry[] = rawReceives.map((entry) => {
      const e = entry as Record<string, unknown>;
      const message = e.message as { $ref: string };
      return {
        message,
        emits: toRefList(e.emits),
        throws: toRefList(e.throws),
        ...(typeof e.effect === 'string' ? { effect: e.effect } : {}),
      };
    });
    actors.push({
      name,
      type,
      ...(typeof node.description === 'string' ? { description: node.description } : {}),
      receives,
    });
  }
  return actors;
}

function asStringList(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

/** A column `type` is either an inline SQL primitive string or a `$ref` into scalars.yaml. */
function columnType(raw: unknown): string {
  if (raw !== null && typeof raw === 'object' && typeof (raw as Record<string, unknown>).$ref === 'string') {
    return String((raw as Record<string, unknown>).$ref).split('#/')[1] ?? '';
  }
  return String(raw ?? '');
}

/**
 * A foreign key is `"View_Name.column"`. It may be written as that string, or as a `$ref` binding
 * `{ $ref: '#/View_Name/columns/column' }` (preferred) — both normalise to `"View_Name.column"`.
 */
function parseFk(raw: unknown): string | undefined {
  if (typeof raw === 'string') return raw;
  if (raw !== null && typeof raw === 'object' && typeof (raw as Record<string, unknown>).$ref === 'string') {
    const segs = String((raw as Record<string, unknown>).$ref).split('#/')[1]?.split('/').filter(Boolean) ?? [];
    if (segs.length >= 2) return `${segs[0]}.${segs[segs.length - 1]}`; // View_X / columns / col → View_X.col
  }
  return undefined;
}

/** `from` is a list of `$ref`s into events.yaml (a whole event or an event property). */
function fromRefs(raw: unknown): string[] {
  if (!Array.isArray(raw)) return [];
  return raw
    .map((it) => (it !== null && typeof it === 'object' && typeof (it as Record<string, unknown>).$ref === 'string' ? String((it as Record<string, unknown>).$ref) : ''))
    .filter((s) => s.length > 0);
}

/** Map an events.yaml property schema node to the column type it implies. */
function schemaNodeToColumnType(node: Record<string, unknown>): string {
  if (typeof node.$ref === 'string') {
    const [file, name] = [node.$ref.split('#/')[0], node.$ref.split('#/')[1] ?? ''];
    return file === 'scalars.yaml' ? name : 'jsonb'; // entity/value-object → stored as jsonb
  }
  if (node.type === 'array') return 'jsonb';
  if (node.type === 'integer') return 'integer';
  if (node.type === 'number') return 'numeric';
  if (node.type === 'boolean') return 'boolean';
  if (node.type === 'string') return node.format === 'date-time' ? 'timestamptz' : 'text';
  return 'text';
}

/** Derive a column type from the first `from` entry that points at a typed event PROPERTY. */
function deriveType(refs: string[], eventsDefs: Record<string, SchemaNode>): string {
  for (const ref of refs) {
    const segs = (ref.split('#/')[1] ?? '').split('/').filter(Boolean); // [Event, 'properties', field]
    if (segs.length < 3 || segs[1] !== 'properties') continue;
    const ev = eventsDefs[segs[0] ?? ''] as Record<string, unknown> | undefined;
    const props = (ev?.properties ?? {}) as Record<string, unknown>;
    const node = props[segs[2] ?? ''] as Record<string, unknown> | undefined;
    if (node) return schemaNodeToColumnType(node);
  }
  return '';
}

function parseColumn(name: string, raw: unknown, eventsDefs: Record<string, SchemaNode>): ViewColumn {
  const col = (raw ?? {}) as Record<string, unknown>;
  const fk = parseFk(col.fk);
  const from = fromRefs(col.from);
  // Explicit `type` wins; otherwise derive it from the source event property (`from`).
  const hasExplicit = col.type !== undefined && col.type !== null;
  const type = hasExplicit ? columnType(col.type) : deriveType(from, eventsDefs);
  return {
    name,
    type,
    ...(!hasExplicit && type ? { typeDerived: true } : {}),
    ...(from.length ? { from } : {}),
    ...(col.pk === true ? { pk: true } : {}),
    ...(col.unique === true ? { unique: true } : {}),
    ...(col.index === true ? { index: true } : {}),
    ...(col.nullable === true ? { nullable: true } : {}),
    ...(fk ? { fk } : {}),
    ...(typeof col.note === 'string' ? { note: col.note } : {}),
  };
}

function parseViews(defs: Record<string, SchemaNode>, eventsDefs: Record<string, SchemaNode>): View[] {
  const views: View[] = [];
  for (const [name, node] of Object.entries(defs)) {
    if (typeof node.aggregate !== 'string') continue; // a view always names its aggregate
    // `columns` is a map keyed by column name (the column name is the key); an array of
    // `{ name, ... }` entries is still accepted for backward compatibility.
    let columns: ViewColumn[];
    if (Array.isArray(node.columns)) {
      columns = node.columns.map((c) => parseColumn(String((c as Record<string, unknown>).name ?? ''), c, eventsDefs));
    } else if (node.columns !== null && typeof node.columns === 'object') {
      columns = Object.entries(node.columns as Record<string, unknown>).map(([colName, c]) => parseColumn(colName, c, eventsDefs));
    } else {
      columns = [];
    }
    const indexes: string[][] = (Array.isArray(node.indexes) ? node.indexes : []).map((ix) => asStringList(ix));
    views.push({
      name,
      aggregate: node.aggregate,
      slice: typeof node.slice === 'string' ? node.slice : 'V0',
      ...(node.internal === true ? { internal: true } : {}),
      fedBy: toRefList(node.fedBy),
      filters: asStringList(node.filters),
      rules: asStringList(node.rules),
      indexes,
      columns,
      ...(typeof node.note === 'string' ? { note: node.note } : {}),
    });
  }
  return views;
}

function parseField(name: string, raw: unknown): ApiField {
  const n = (raw ?? {}) as Record<string, unknown>;
  const ref = typeof n.$ref === 'string';
  // The referenced definition name is the LAST `/`-segment — works for cross-file
  // (`entities.yaml#/Address`) and local API-type (`#/types/Product`) refs alike.
  const type = ref ? refOrName(n.$ref) : String(n.type ?? '');
  return {
    name,
    type,
    ref,
    ...(n.required === true ? { required: true } : {}),
    ...(n.nullable === true ? { nullable: true } : {}),
    ...(n.array === true ? { array: true } : {}),
    ...(typeof n.format === 'string' ? { format: n.format } : {}),
  };
}

function fieldMap(raw: unknown): ApiField[] {
  if (typeof raw !== 'object' || raw === null) return [];
  return Object.entries(raw as Record<string, unknown>).map(([name, node]) => parseField(name, node));
}

/**
 * The bare definition name referenced by a value, accepting both forms (so the spec can be written
 * with `$ref` instead of string literals):
 *   - a `$ref` object  `{ $ref: 'views.yaml#/View_Restaurant' }` or `{ $ref: '#/types/Restaurant' }`
 *   - a `$ref` string  `'#/types/Restaurant'`
 *   - a bare name      `'View_Restaurant'`
 * In every case the LAST `/`-separated segment is the name.
 */
function refOrName(value: unknown): string {
  let v = value;
  if (v !== null && typeof v === 'object' && typeof (v as Record<string, unknown>).$ref === 'string') {
    v = (v as Record<string, unknown>).$ref;
  }
  return String(v ?? '').split('/').pop() ?? '';
}

/** A list of view/type names, each element either a `$ref` (object/string) or a bare name. */
function nameList(value: unknown): string[] {
  if (!Array.isArray(value)) return [];
  return value.map(refOrName).filter((s) => s.length > 0);
}

function parseApi(defs: Record<string, SchemaNode>): Api {
  const typesNode = (defs.types ?? {}) as Record<string, Record<string, unknown>>;
  const types: ApiType[] = Object.entries(typesNode).map(([name, t]) => ({
    name,
    ...(typeof t.description === 'string' ? { description: t.description } : {}),
    reads: nameList(t.reads),
    properties: fieldMap(t.properties),
  }));
  // reads are declared ON the type; a query inherits the binding of its return type.
  const readsByType = new Map(types.map((t) => [t.name, t.reads]));

  const queriesNode = (defs.queries ?? {}) as Record<string, Record<string, unknown>>;
  const queries: ApiQuery[] = Object.entries(queriesNode).map(([name, q]) => {
    const returns = (q.returns ?? {}) as Record<string, unknown>;
    // `returns` may name its type via `type: Restaurant` or `$ref: '#/types/Restaurant'`.
    const returnsType = refOrName(returns.$ref ?? returns.type);
    return {
      name,
      ...(typeof q.description === 'string' ? { description: q.description } : {}),
      args: fieldMap(q.args),
      returnsType,
      returnsList: returns.array === true,
      returnsNullable: returns.nullable === true,
      reads: readsByType.get(returnsType) ?? [],
      roles: asStringList(q.roles),
      slice: typeof q.slice === 'string' ? q.slice : 'V0',
    };
  });

  const mutationsNode = (defs.mutations ?? {}) as Record<string, Record<string, unknown>>;
  const mutations: ApiMutation[] = Object.entries(mutationsNode).map(([name, m]) => ({
    name,
    ...(typeof m.description === 'string' ? { description: m.description } : {}),
    // `command` may be a `$ref` into commands.yaml or a bare command name.
    command: refOrName(m.command),
    roles: asStringList(m.roles),
    slice: typeof m.slice === 'string' ? m.slice : 'V0',
    payload: fieldMap(m.payload),
  }));

  return { types, queries, mutations };
}

/** Parse the story map (stories.yaml): personas → activities → steps (each step refs an api op). */
function parseStories(defs: Record<string, SchemaNode>): Persona[] {
  const personas: Persona[] = [];
  for (const [name, node] of Object.entries(defs)) {
    // A persona declares its role and/or its activities.
    if (typeof node.personaRole !== 'string' && (node.activities === undefined || node.activities === null)) continue;
    const activitiesNode = (node.activities ?? {}) as Record<string, Record<string, unknown>>;
    const activities: StoryActivity[] = Object.entries(activitiesNode).map(([aName, a]) => {
      const stepsNode = (a.steps ?? {}) as Record<string, Record<string, unknown>>;
      const steps: StoryStep[] = Object.entries(stepsNode).map(([sName, s]) => {
        const ref = typeof s.$ref === 'string' ? s.$ref : undefined;
        if (ref) {
          const [seg0, op] = (ref.split('#/')[1] ?? '').split('/');
          const opKind = seg0 === 'queries' ? 'query' : seg0 === 'mutations' ? 'mutation' : undefined;
          return { name: sName, ...(opKind ? { opKind } : {}), ...(op ? { op } : {}) };
        }
        return { name: sName, ...(typeof s.note === 'string' ? { note: s.note } : {}) };
      });
      return { name: aName, ...(typeof a.description === 'string' ? { description: a.description } : {}), steps };
    });
    personas.push({
      name,
      ...(typeof node.description === 'string' ? { description: node.description } : {}),
      role: typeof node.personaRole === 'string' ? node.personaRole : '',
      ...(typeof node.locale === 'string' ? { locale: node.locale } : {}),
      activities,
    });
  }
  return personas;
}

/** Load and parse the full spec model from a `specs/` directory. */
export function loadModel(specsDir: string): Model {
  const defs = {} as Model['defs'];
  const meta = {} as Model['meta'];
  for (const file of SOURCE_FILES) {
    const loaded = loadFile(specsDir, file);
    defs[file] = loaded.defs;
    meta[file] = loaded.meta;
  }
  const nonProjected = defs['views.yaml']['nonProjectedEvents'];
  const nonProjectedEvents = (Array.isArray(nonProjected) ? nonProjected : [])
    .map((r) => {
      const ref = (r as Record<string, unknown>).$ref;
      return typeof ref === 'string' ? ref.split('#/')[1] : undefined;
    })
    .filter((n): n is string => typeof n === 'string');

  return {
    defs,
    meta,
    actors: parseActors(defs['actors.yaml']),
    views: parseViews(defs['views.yaml'], defs['events.yaml']),
    nonProjectedEvents,
    api: parseApi(defs['api.yaml']),
    personas: parseStories(defs['stories.yaml']),
  };
}
