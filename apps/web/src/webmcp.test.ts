import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PlanogramSession } from './session';
import { registerPlanogramWebMcp } from './webmcp';
import { PERFORMANCE_SOURCE, VERSION_ID, adjustableShelf, appliedResult, baseDeck, changeSetId, loosePlacement, looseProduct, makeContext, readyPreview, reflowOperation, revisionConflict, trayPlacement, trayProduct } from './testFixtures';
import type { CommandResult, EngineContext, PreviewResult, WasmEngine } from './types';

interface RegisteredTool {
  name: string;
  annotations?: { readOnlyHint?: boolean };
  inputSchema: { additionalProperties?: boolean };
  execute: (args: unknown, context?: { signal?: AbortSignal }) => unknown | Promise<unknown>;
}

let activeContext: EngineContext;
let registered: RegisteredTool[];

/** Returns a revision conflict unless `expectedRevision` matches, mirroring the Rust guard. */
function guarded<T extends CommandResult | PreviewResult>(expectedRevision: number, run: () => T): T | ReturnType<typeof revisionConflict> {
  return expectedRevision === activeContext.revision ? run() : revisionConflict(expectedRevision, activeContext.revision);
}

/** Advances the fake draft one revision and returns a canned applied result. */
function commit(actor: string, reason: string, changes: Partial<EngineContext> = {}, operations: unknown[] = [], affectedIds: string[] = []) {
  const revision = activeContext.revision + 1;
  activeContext = { ...activeContext, latest_change_set_id: changeSetId(revision), latest_undoable_change_set_id: changeSetId(revision), ...changes, revision };
  return appliedResult(revision, { actor, reason, operations, affectedIds });
}

// Canned Wasm responses: these tests cover argument transport and result shaping, not Rust geometry.
const engine = {
  context: vi.fn(() => activeContext),
  validate_planogram: vi.fn(() => ({ revision: activeContext.revision, valid: true, validation: { issues: [] } })),
  add_placement_as: vi.fn((_versionId: string, productId: string, shelfId: string, expectedRevision: number, actor: string, reason: string) => guarded(expectedRevision, () => {
    const placement = trayPlacement({ product_id: productId, shelf_id: shelfId });
    return commit(actor, reason, { placements: [placement] }, [{ type: 'add_placement', placement }], [placement.id]);
  })),
  undo_change_set_as: vi.fn((_versionId: string, changeSetId: string, expectedRevision: number, actor: string) => guarded(expectedRevision, () =>
    commit(actor, `Undo ${changeSetId}`, { placements: [], latest_undoable_change_set_id: undefined }))),
  distribute_shelf_as: vi.fn((_versionId: string, _shelfId: string, _distribution: string, expectedRevision: number, actor: string, reason: string) => guarded(expectedRevision, () =>
    commit(actor, reason, {}, [], activeContext.placements.map(placement => placement.id)))),
  set_facings_as: vi.fn((_versionId: string, placementId: string, facingsX: number | undefined, facingsY: number | undefined, facingsZ: number | undefined, expectedRevision: number, actor: string, reason: string) => guarded(expectedRevision, () => {
    const current = activeContext.placements.find(placement => placement.id === placementId)!;
    const counts = { facings_x: facingsX ?? current.facings_x, facings_y: facingsY ?? current.facings_y, facings_z: facingsZ ?? current.facings_z };
    const updated = { ...current, ...counts };
    const operation = { type: 'change_facings', placement_id: placementId, before: { facings_x: current.facings_x, facings_y: current.facings_y, facings_z: current.facings_z }, after: counts };
    return commit(actor, reason, { placements: [updated] }, [operation], [placementId]);
  })),
  preview_changes: vi.fn((_versionId: string, expectedRevision: number) => guarded(expectedRevision, () =>
    readyPreview(activeContext.revision, [{ type: 'add_placement', placement: trayPlacement() }]))),
  preview_shelf_allocation: vi.fn((_versionId: string, _shelfId: string, _strategy: string, expectedRevision: number) => guarded(expectedRevision, () =>
    readyPreview(activeContext.revision, [reflowOperation(activeContext.placements[0], { x: 4, facings_x: 4 })]))),
  clear_proposal_preview: vi.fn(),
  apply_changes_as: vi.fn((_versionId: string, expectedRevision: number, _changes: unknown[], actor: string, reason: string) => guarded(expectedRevision, () => {
    const placement = trayPlacement();
    return commit(actor, reason, { placements: [placement] }, [{ type: 'add_placement', placement }], [placement.id]);
  })),
  apply_shelf_allocation_as: vi.fn((_versionId: string, _shelfId: string, _strategy: string, expectedRevision: number, actor: string, reason: string) => guarded(expectedRevision, () => {
    const current = activeContext.placements[0];
    return commit(actor, reason, {}, [reflowOperation(current, { x: 4, facings_x: 4 })], [current.id]);
  })),
};

function tool(name: string): RegisteredTool {
  const found = registered.find(candidate => candidate.name === name);
  if (!found) throw new Error(`Missing registered tool ${name}`);
  return found;
}

function execute(name: string, args: unknown = {}) {
  return Promise.resolve(tool(name).execute(args, { signal: new AbortController().signal }));
}

describe('WebMCP site tools', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    activeContext = makeContext({ shelves: [baseDeck, adjustableShelf('shelf_01', 192)], products: [trayProduct] });
    registered = [];
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: vi.fn((registeredTool: RegisteredTool) => { registered.push(registeredTool); }),
        unregisterTool: vi.fn(),
      },
    });
    const session = new PlanogramSession(engine as unknown as WasmEngine, { onContext: vi.fn(), onCommand: vi.fn() });
    const registration = await registerPlanogramWebMcp(session, () => undefined);
    expect(registration.status).toBe('ready');
  });

  it('registers strict read and write tools only after WebMCP support is present', () => {
    expect(registered).toHaveLength(12);
    expect(registered.every(registeredTool => registeredTool.inputSchema.additionalProperties === false)).toBe(true);
    for (const name of ['get_planogram_context', 'validate_planogram', 'preview_shelf_allocation', 'preview_changes']) {
      expect(tool(`planogram.${name}`).annotations?.readOnlyHint).toBe(true);
    }
    for (const name of ['add_product', 'distribute_shelf', 'set_facings', 'apply_changes']) {
      expect(tool(`planogram.${name}`).annotations).toBeUndefined();
    }
  });

  it('executes when the browser runtime omits the optional execution context', async () => {
    expect(await tool('planogram.get_planogram_context').execute({})).toMatchObject({ status: 'ok', revision: 0 });
  });

  it('answers context, catalog, section, and validation queries without mutation', async () => {
    expect(await execute('planogram.get_planogram_context')).toMatchObject({
      status: 'ok',
      revision: 0,
      fixture: { width_sixteenths: 768 },
      selection: null,
      summary: { latest_change_set_id: null, latest_undoable_change_set_id: null },
    });
    expect(await execute('planogram.search_products', { query: 'jif 16 oz' })).toMatchObject({ status: 'ok', revision: 0, products: [{ id: 'jif_creamy_16' }] });
    expect(await execute('planogram.search_products', { stocking_mode: 'tray' })).toMatchObject({ status: 'ok', products: [{ id: 'jif_creamy_16', tray: { facings_x: 3, units_deep: 4 } }] });
    expect(await execute('planogram.search_products', { stocking_mode: 'loose' })).toMatchObject({ status: 'ok', products: [] });
    expect(await execute('planogram.get_product', { product_id: 'jif_creamy_16' })).toMatchObject({
      status: 'ok',
      revision: 0,
      product: { id: 'jif_creamy_16', dimensions: { depth_sixteenths: 45 }, performance: { period: 'Trailing 13 weeks', source: PERFORMANCE_SOURCE }, tray: { facings_x: 3, units_deep: 4 } },
    });
    expect(await execute('planogram.get_section', { section_id: 'section_01' })).toMatchObject({
      status: 'ok',
      revision: 0,
      section: { shelves: [{ id: 'base_deck' }, { id: 'shelf_01', available_capacity_sixteenths: 768 }] },
    });

    const beforeValidation = activeContext;
    expect(await execute('planogram.validate_planogram')).toEqual({ status: 'ok', revision: 0, valid: true, validation: { issues: [] } });
    expect(activeContext).toBe(beforeValidation);
    expect(await execute('planogram.validate_planogram', { unexpected: true })).toMatchObject({ status: 'error', code: 'invalid_input', revision: 0 });
    expect(await execute('planogram.get_product', { product_id: 'jif_creamy_16', x: 1 })).toMatchObject({ status: 'error', code: 'invalid_input', revision: 0 });
    expect(engine.validate_planogram).toHaveBeenCalledOnce();
  });

  it('applies an attributed add with stale protection and undo', async () => {
    expect(await execute('planogram.add_product', { product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0, reason: 'Agent assortment pass' })).toMatchObject({
      status: 'applied',
      revision: 1,
      placement: { id: 'placement_0001', x_sixteenths: 0, stocking_mode: 'tray', stocked_unit_count: 12, display_width_sixteenths: 175, required_depth_sixteenths: 232 },
      change_set: { actor: 'webmcp', reason: 'Agent assortment pass', operations: [{ type: 'add_placement', placement: { x_sixteenths: 0, facings_x: 3, facings_z: 4 } }] },
    });
    expect(engine.add_placement_as).toHaveBeenCalledWith(VERSION_ID, 'jif_creamy_16', 'shelf_01', 0, 'webmcp', 'Agent assortment pass');
    expect(await execute('planogram.get_planogram_context')).toMatchObject({ summary: { latest_change_set_id: 'change_0001', latest_undoable_change_set_id: 'change_0001' } });

    expect(await execute('planogram.add_product', { product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0 })).toMatchObject({ status: 'revision_conflict', expected_revision: 0, current_revision: 1 });

    expect(await execute('planogram.undo_change_set', { change_set_id: 'change_0001', expected_revision: 1 })).toMatchObject({ status: 'applied', revision: 2, change_set: { actor: 'webmcp' } });
    expect(activeContext.placements).toHaveLength(0);
    expect(await execute('planogram.get_planogram_context')).toMatchObject({ summary: { latest_change_set_id: 'change_0002', latest_undoable_change_set_id: null } });
  });

  it('returns a structured cancellation without invoking a command', async () => {
    const controller = new AbortController();
    controller.abort();
    const result = await tool('planogram.add_product').execute({ product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0 }, { signal: controller.signal });
    expect(result).toMatchObject({ status: 'error', code: 'cancelled', revision: 0 });
    expect(engine.add_placement_as).not.toHaveBeenCalled();
  });

  it('passes semantic shelf distribution to Rust and rejects unknown layout modes', async () => {
    activeContext = { ...activeContext, revision: 4, placements: [trayPlacement(), trayPlacement({ id: 'placement_0002', x: 180 })] };

    expect(await execute('planogram.distribute_shelf', { shelf_id: 'shelf_01', distribution: 'random', expected_revision: 4 })).toMatchObject({ status: 'error', code: 'invalid_input', revision: 4 });
    expect(engine.distribute_shelf_as).not.toHaveBeenCalled();

    expect(await execute('planogram.distribute_shelf', { shelf_id: 'shelf_01', distribution: 'space_evenly', expected_revision: 4, reason: 'Balance the shelf' })).toMatchObject({ status: 'applied', revision: 5, affected_ids: ['placement_0001', 'placement_0002'] });
    expect(engine.distribute_shelf_as).toHaveBeenCalledWith(VERSION_ID, 'shelf_01', 'space_evenly', 4, 'webmcp', 'Balance the shelf');
  });

  it('routes semantic facing counts to Rust and leaves omitted counts unresolved', async () => {
    activeContext = { ...activeContext, revision: 4, products: [trayProduct, looseProduct], placements: [loosePlacement()] };

    for (const args of [
      { placement_id: 'placement_0002', facings_x: 0, expected_revision: 4 },
      { placement_id: 'placement_0002', facings_x: 2.5, expected_revision: 4 },
      { placement_id: 'placement_0002', facings_x: 2, x_sixteenths: 10, expected_revision: 4 },
    ]) {
      expect(await execute('planogram.set_facings', args)).toMatchObject({ status: 'error', code: 'invalid_input', revision: 4 });
    }
    expect(engine.set_facings_as).not.toHaveBeenCalled();

    expect(await execute('planogram.set_facings', { placement_id: 'placement_0002', facings_x: 3, expected_revision: 4, reason: 'Match facings to movement' })).toMatchObject({
      status: 'applied',
      revision: 5,
      placement: { id: 'placement_0002', facings_x: 3, facings_y: 1, facings_z: 1 },
      change_set: { actor: 'webmcp', reason: 'Match facings to movement', operations: [{ type: 'change_facings', placement_id: 'placement_0002', before: { facings_x: 1 }, after: { facings_x: 3 } }] },
    });
    expect(engine.set_facings_as).toHaveBeenCalledWith(VERSION_ID, 'placement_0002', 3, undefined, undefined, 4, 'webmcp', 'Match facings to movement');

    expect(await execute('planogram.set_facings', { placement_id: 'placement_0002', facings_y: 2, expected_revision: 4 })).toMatchObject({ status: 'revision_conflict', expected_revision: 4, current_revision: 5 });
  });

  it('previews and applies Rust-owned shelf-facing allocation without model-supplied coordinates', async () => {
    activeContext = { ...activeContext, revision: 4, placements: [trayPlacement()] };

    expect(await execute('planogram.preview_shelf_allocation', { shelf_id: 'shelf_01', strategy: 'invent_positions', expected_revision: 4 })).toMatchObject({ status: 'error', code: 'invalid_input', revision: 4 });
    expect(engine.preview_shelf_allocation).not.toHaveBeenCalled();

    expect(await execute('planogram.preview_shelf_allocation', { shelf_id: 'shelf_01', strategy: 'fill_evenly', expected_revision: 4, reason: 'Fill the shelf evenly' })).toMatchObject({
      status: 'ready',
      revision: 4,
      proposal_id: 'proposal_0001',
      operations: [{ type: 'reflow_placement', before: { x_sixteenths: 0, facings_x: 3 }, after: { x_sixteenths: 4, facings_x: 4 } }],
    });
    expect(activeContext.revision).toBe(4);
    expect(engine.preview_shelf_allocation).toHaveBeenCalledWith(VERSION_ID, 'shelf_01', 'fill_evenly', 4);

    expect(await execute('planogram.apply_changes', { proposal_id: 'proposal_0001', expected_revision: 4 })).toMatchObject({
      status: 'applied',
      revision: 5,
      change_set: { actor: 'webmcp', reason: 'Fill the shelf evenly', operations: [{ type: 'reflow_placement' }] },
    });
    expect(engine.apply_shelf_allocation_as).toHaveBeenCalledWith(VERSION_ID, 'shelf_01', 'fill_evenly', 4, 'webmcp', 'Fill the shelf evenly');
  });

  it('rejects model-supplied final coordinates in favor of semantic sequence', async () => {
    const result = await execute('planogram.preview_changes', {
      expected_revision: 0,
      operations: [{ kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', x_sixteenths: 0 }],
    });
    expect(result).toMatchObject({ status: 'error', code: 'invalid_input', revision: 0 });
    expect(engine.preview_changes).not.toHaveBeenCalled();
  });

  it('previews generic placement operations without mutation and applies the reviewed proposal atomically', async () => {
    const add = { kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 };
    expect(await execute('planogram.preview_changes', { expected_revision: 0, reason: 'Group the Jif family by size', operations: [add] })).toMatchObject({
      status: 'ready',
      revision: 0,
      proposal_id: 'proposal_0001',
      reason: 'Group the Jif family by size',
      operations: [{ type: 'add_placement', placement: { id: 'placement_0001', x_sixteenths: 0, facings_x: 3, facings_z: 4 } }],
    });
    expect(activeContext.revision).toBe(0);

    expect(await execute('planogram.apply_changes', { proposal_id: 'proposal_0001', expected_revision: 0 })).toMatchObject({
      status: 'applied',
      revision: 1,
      placements: [{ id: 'placement_0001' }],
      change_set: { actor: 'webmcp', reason: 'Group the Jif family by size' },
    });
    expect(engine.preview_changes).toHaveBeenCalledWith(VERSION_ID, 0, [add]);
    expect(engine.apply_changes_as).toHaveBeenCalledWith(VERSION_ID, 0, [add], 'webmcp', 'Group the Jif family by size');
  });
});
