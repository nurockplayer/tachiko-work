/** Private Browser presentation state. Collections are stable schema IDs. */
export type BudgetView = { id: string; name: string; collection: string };
export type BudgetViews = { version: 1; views: BudgetView[]; active: string };

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const fail = (message: string): never => { throw new Error(`Invalid Budget views: ${message}`); };
const object = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const validName = (name: unknown): name is string =>
  typeof name === "string" && name.trim().length > 0 && Array.from(name).length <= 80 &&
  !Array.from(name).some(character => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127);

export function parseBudgetViews(input: unknown, collections: string[]): BudgetViews {
  if (!object(input) || input.version !== 1 || !Array.isArray(input.views) || input.views.length < 1 || input.views.length > 32)
    return fail("expected version 1 with between 1 and 32 views.");
  if (Object.keys(input).some(key => !["version", "views", "active"].includes(key)))
    return fail("unsupported state property.");
  const seen = new Set<string>();
  const views = input.views.map((item: unknown): BudgetView => {
    if (!object(item) || typeof item.id !== "string" || !UUID.test(item.id) || seen.has(item.id))
      return fail("view IDs must be unique UUID v4 strings.");
    if (!validName(item.name)) return fail("names must contain between 1 and 80 characters without control characters.");
    if (typeof item.collection !== "string" || !collections.includes(item.collection))
      return fail("view references an unavailable collection ID.");
    if (Object.keys(item).some(key => !["id", "name", "collection"].includes(key)))
      return fail("unsupported view property.");
    seen.add(item.id);
    return { id: item.id, name: item.name, collection: item.collection };
  });
  if (typeof input.active !== "string" || !seen.has(input.active)) return fail("active view is unavailable.");
  return { version: 1, views, active: input.active };
}

/** Call only when an occurrence has no saved view state; persist the generated IDs. */
export function defaultBudgetViews(collections: string[]): BudgetViews {
  if (collections.length < 1 || collections.length > 32 || new Set(collections).size !== collections.length || collections.some(id => id.length === 0))
    return fail("defaults require between 1 and 32 distinct collection IDs.");
  const views = collections.map((collection, index) => ({ id: crypto.randomUUID(), name: `Sheet ${String(index + 1)}`, collection }));
  return parseBudgetViews({ version: 1, views, active: views[0]?.id }, collections);
}

function copy(state: BudgetViews): BudgetViews {
  return parseBudgetViews(state, state.views.map(view => view.collection));
}

function position(state: BudgetViews, id: string): number {
  const index = state.views.findIndex(view => view.id === id);
  return index < 0 ? fail("requested view is unavailable.") : index;
}

export function addBudgetView(state: BudgetViews, view: BudgetView, collections: string[]): BudgetViews {
  const next = parseBudgetViews(state, collections);
  next.views.push({ ...view });
  next.active = view.id;
  return parseBudgetViews(next, collections);
}

/** Duplicates a projection, sharing its collection; never copies semantic data. */
export function duplicateBudgetView(state: BudgetViews, sourceId: string, newId: string, name: string, collections: string[]): BudgetViews {
  const next = parseBudgetViews(state, collections);
  const source = next.views[position(next, sourceId)];
  if (!source) return fail("requested view is unavailable.");
  return addBudgetView(next, { id: newId, name, collection: source.collection }, collections);
}

export function renameBudgetView(state: BudgetViews, id: string, name: string): BudgetViews {
  const next = copy(state);
  if (!validName(name)) return fail("names must contain between 1 and 80 characters without control characters.");
  const view = next.views[position(next, id)];
  if (view) view.name = name;
  return next;
}

/** The permutation must include every existing ID exactly once. */
export function reorderBudgetViews(state: BudgetViews, ids: string[]): BudgetViews {
  const next = copy(state);
  if (ids.length !== next.views.length || new Set(ids).size !== ids.length)
    return fail("reordering requires each view exactly once.");
  next.views = ids.map(id => {
    const view = next.views[position(next, id)];
    return view ?? fail("requested view is unavailable.");
  });
  return next;
}

/** Removes only a presentation tab. The underlying collection remains intact. */
export function deleteBudgetView(state: BudgetViews, id: string): BudgetViews {
  const next = copy(state);
  const index = position(next, id);
  if (next.views.length === 1) return fail("at least one view must remain.");
  next.views.splice(index, 1);
  if (next.active === id) next.active = next.views[Math.min(index, next.views.length - 1)]?.id ?? fail("active view is unavailable.");
  return next;
}
