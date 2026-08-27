import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PlanogramSession } from './session';
import { registerPlanogramWebMcp } from './webmcp';
import type { CommandResult, EngineContext, Product, WasmEngine } from './types';

interface RegisteredTool {
  name: string;
  annotations?: { readOnlyHint?: boolean };
  inputSchema: { additionalProperties?: boolean };
  execute: (args: unknown, context?: { signal?: AbortSignal }) => unknown | Promise<unknown>;
}

const product: Product = {
  id: 'jif_creamy_16',
  upc: '051500255001',
  brand: 'Jif',
  description: 'Creamy Peanut Butter',
  size_oz: '16 oz',
  category: 'Peanut Butter',
  dimensions: { width: 57, height: 130, depth: 45, source: 'fixture', confidence: 'high' },
  net_weight_ounces_hundredths: 1600,
  casepack_quantity: 12,
  performance: {
    sales_per_store_per_week_cents: 3665,
    units_per_store_per_week_milliunits: 10500,
    gross_margin_basis_points: 2850,
    source: 'Synthetic representative 13-week average; not retailer actuals',
    period: 'Trailing 13 weeks',
  },
  tray: {
    facings_x: 3,
    units_deep: 4,
    outer_width_sixteenths: 175,
    outer_height_sixteenths: 80,
    outer_depth_sixteenths: 232,
    front_lip_height_sixteenths: 20,
  },
  color: [40, 100, 60],
  lid_color: [210, 30, 40],
};

const contextFixture = {
  id: 'fixture_standard_4ft',
  name: "4' Standard Bay",
  width: 768,
  height: 1344,
  depth: 352,
  sections: [{
    id: 'section_01',
    fixture_id: 'fixture_standard_4ft',
    sequence: 0,
    width: 768,
    height: 1344,
    shelves: [
      { id: 'base_deck', section_id: 'section_01', kind: 'base_deck' as const, width: 768, depth: 352, elevation: 0 },
      { id: 'shelf_01', section_id: 'section_01', kind: 'adjustable' as const, width: 768, depth: 256, elevation: 192 },
    ],
  }],
};

function makeContext(): EngineContext {
  return {
    version_id: 'version_draft_01',
    version_status: 'draft',
    revision: 0,
    fixture: contextFixture,
    products: [product],
    placements: [],
    latest_change_set_id: undefined,
  };
}

function placementView(id = 'placement_0001', shelfId = 'shelf_01', x = 0): EngineContext['placements'][number] {
  return {
    id,
    product_id: product.id,
    shelf_id: shelfId,
    x,
    stocking_mode: 'tray',
    facings_x: 3,
    facings_y: 1,
    facings_z: 4,
    stocked_unit_count: 12,
    geometry: { display_width: 175, display_height: 80, required_depth: 232 },
    tray_front_lip_height: 20,
  };
}

function appliedResult(revision: number, actor: string, reason: string, changeSetId: string, placement?: EngineContext['placements'][number]): CommandResult {
  return {
    status: 'applied',
    revision,
    change_set: {
      id: changeSetId,
      actor,
      reason,
      base_revision: revision - 1,
      resulting_revision: revision,
      operations: placement ? [{ type: 'add_placement', placement: { ...placement } }] : [],
    },
    affected_ids: placement ? [placement.id] : [],
    validation: { issues: [] },
    scene_patch: { revision, shelves: [], placements: placement ? [{}] : [], removed_placement_ids: [] },
  };
}

function toolByName(tools: RegisteredTool[], name: string): RegisteredTool {
  const tool = tools.find(candidate => candidate.name === name);
  if (!tool) throw new Error(`Missing registered tool ${name}`);
  return tool;
}

describe('WebMCP site tools', () => {
  let activeContext: EngineContext;
  let engine: WasmEngine;
  let registered: RegisteredTool[];

  beforeEach(() => {
    activeContext = makeContext();
    const add_placement_as = vi.fn((_versionId: string, productId: string, shelfId: string, expectedRevision: number, actor: string, reason: string) => {
      if (expectedRevision !== activeContext.revision) return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: activeContext.revision } satisfies CommandResult;
      const placement = { ...placementView('placement_0001', shelfId), product_id: productId };
      activeContext = { ...activeContext, revision: 1, placements: [placement], latest_change_set_id: 'change_0001' };
      return appliedResult(1, actor, reason, 'change_0001', placement);
    });
    const undo_change_set_as = vi.fn((_versionId: string, _changeSetId: string, expectedRevision: number, actor: string) => {
      if (expectedRevision !== activeContext.revision) return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: activeContext.revision } satisfies CommandResult;
      activeContext = { ...activeContext, revision: 2, placements: [], latest_change_set_id: 'change_0002' };
      return appliedResult(2, actor, 'Undo change_0001', 'change_0002');
    });
    const distribute_shelf_as = vi.fn((_versionId: string, shelfId: string, _distribution: string, expectedRevision: number, actor: string, reason: string) => {
      if (expectedRevision !== activeContext.revision) return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: activeContext.revision } satisfies CommandResult;
      const revision = activeContext.revision + 1;
      const placements = activeContext.placements.map((placement, index) => ({ ...placement, shelf_id: shelfId, x: 120 + index * 180 }));
      activeContext = { ...activeContext, revision, placements, latest_change_set_id: `change_${String(revision).padStart(4, '0')}` };
      const result = appliedResult(revision, actor, reason, activeContext.latest_change_set_id!);
      return { ...result, affected_ids: placements.map(placement => placement.id) };
    });
    const preview_changes = vi.fn((_versionId: string, expectedRevision: number, changes: Array<{ kind: string }>) => {
      if (expectedRevision !== activeContext.revision) return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: activeContext.revision };
      const placement = placementView();
      return {
        status: 'ready',
        revision: activeContext.revision,
        operations: changes.map(change => change.kind === 'add' ? { type: 'add_placement', placement } : { type: 'unknown' }),
        affected_ids: [placement.id],
        validation: { issues: [] },
        preview_scene: {
          revision: activeContext.revision,
          fixture_id: activeContext.fixture.id,
          width: activeContext.fixture.width,
          height: activeContext.fixture.height,
          shelves: activeContext.fixture.sections.flatMap(section => section.shelves),
          placements: [{
            id: placement.id,
            product_id: placement.product_id,
            shelf_id: placement.shelf_id,
            x: placement.x,
            width: placement.geometry.display_width,
            height: placement.geometry.display_height,
            required_depth: placement.geometry.required_depth,
            stocking_mode: placement.stocking_mode,
            stocked_unit_count: placement.stocked_unit_count,
            facings_x: placement.facings_x,
            facings_y: placement.facings_y,
            facings_z: placement.facings_z,
            tray_front_lip_height: placement.tray_front_lip_height,
            color: product.color,
            lid_color: product.lid_color,
          }],
        },
      };
    });
    const apply_changes_as = vi.fn((_versionId: string, expectedRevision: number, _changes: unknown[], actor: string, reason: string) => {
      if (expectedRevision !== activeContext.revision) return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: activeContext.revision } satisfies CommandResult;
      const placement = placementView();
      activeContext = { ...activeContext, revision: activeContext.revision + 1, placements: [placement], latest_change_set_id: 'change_0001' };
      return appliedResult(activeContext.revision, actor, reason, 'change_0001', placement);
    });
    engine = {
      context: vi.fn(() => activeContext),
      validate_planogram: vi.fn(() => ({ revision: activeContext.revision, valid: true, validation: { issues: [] } })),
      add_placement_as,
      distribute_shelf_as,
      undo_change_set_as,
      preview_changes,
      clear_proposal_preview: vi.fn(),
      apply_changes_as,
    } as unknown as WasmEngine;
    registered = [];
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: vi.fn((tool: RegisteredTool) => { registered.push(tool); }),
        unregisterTool: vi.fn(),
      },
    });
  });

  it('registers strict read and write tools only after WebMCP support is present', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    const registration = await registerPlanogramWebMcp(session, () => ({ kind: 'shelf', id: 'shelf_01' }));

    expect(registration.status).toBe('ready');
    expect(registration.registeredNames).toHaveLength(10);
    expect(registered.every(tool => tool.inputSchema.additionalProperties === false)).toBe(true);
    expect(toolByName(registered, 'planogram.get_planogram_context').annotations?.readOnlyHint).toBe(true);
    expect(toolByName(registered, 'planogram.validate_planogram').annotations?.readOnlyHint).toBe(true);
    expect(toolByName(registered, 'planogram.add_product').annotations).toBeUndefined();
    expect(toolByName(registered, 'planogram.distribute_shelf').annotations).toBeUndefined();
    expect(toolByName(registered, 'planogram.preview_changes').annotations?.readOnlyHint).toBe(true);
    expect(toolByName(registered, 'planogram.apply_changes').annotations).toBeUndefined();
  });

  it('executes when the browser runtime omits the optional execution context', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);

    expect(await toolByName(registered, 'planogram.get_planogram_context').execute({})).toMatchObject({
      status: 'ok',
      revision: 0,
    });
  });

  it('answers context/catalog/section queries and applies an attributed add with stale protection and undo', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);
    const signal = new AbortController().signal;
    const contextResult = await toolByName(registered, 'planogram.get_planogram_context').execute({}, { signal });
    expect(contextResult).toMatchObject({ status: 'ok', revision: 0, fixture: { width_sixteenths: 768 }, selection: null });

    const searchResult = await toolByName(registered, 'planogram.search_products').execute({ query: 'jif 16 oz' }, { signal });
    expect(searchResult).toMatchObject({
      status: 'ok',
      revision: 0,
      products: [{
        id: 'jif_creamy_16',
        net_weight_ounces_hundredths: 1600,
        casepack_quantity: 12,
        dimensions: { width_sixteenths: 57 },
        performance: { sales_per_store_per_week_cents: 3665, units_per_store_per_week_milliunits: 10500, gross_margin_basis_points: 2850 },
        tray: { outer_width_sixteenths: 175, outer_height_sixteenths: 80, outer_depth_sixteenths: 232, front_lip_height_sixteenths: 20 },
      }],
    });

    const productResult = await toolByName(registered, 'planogram.get_product').execute({ product_id: 'jif_creamy_16' }, { signal });
    expect(productResult).toMatchObject({ status: 'ok', revision: 0, product: { id: 'jif_creamy_16', dimensions: { depth_sixteenths: 45 }, performance: { period: 'Trailing 13 weeks', source: 'Synthetic representative 13-week average; not retailer actuals' }, tray: { facings_x: 3, units_deep: 4 } } });

    const traySearchResult = await toolByName(registered, 'planogram.search_products').execute({ stocking_mode: 'tray' }, { signal });
    expect(traySearchResult).toMatchObject({ status: 'ok', products: [{ id: 'jif_creamy_16', tray: { facings_x: 3, units_deep: 4 } }] });
    const looseSearchResult = await toolByName(registered, 'planogram.search_products').execute({ stocking_mode: 'loose' }, { signal });
    expect(looseSearchResult).toMatchObject({ status: 'ok', products: [] });

    const sectionResult = await toolByName(registered, 'planogram.get_section').execute({ section_id: 'section_01' }, { signal });
    expect(sectionResult).toMatchObject({ status: 'ok', revision: 0, section: { shelves: [{ id: 'base_deck' }, { id: 'shelf_01', available_capacity_sixteenths: 768 }] } });

    const beforeValidation = activeContext;
    const validationResult = await toolByName(registered, 'planogram.validate_planogram').execute({}, { signal });
    expect(validationResult).toEqual({ status: 'ok', revision: 0, valid: true, validation: { issues: [] } });
    expect(activeContext).toBe(beforeValidation);
    expect((engine.validate_planogram as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledOnce();
    const invalidValidation = await toolByName(registered, 'planogram.validate_planogram').execute({ unexpected: true }, { signal });
    expect(invalidValidation).toMatchObject({ status: 'error', code: 'invalid_input', revision: 0 });
    expect((engine.validate_planogram as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledOnce();

    const addResult = await toolByName(registered, 'planogram.add_product').execute({ product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0, reason: 'Agent assortment pass' }, { signal });
    expect(addResult).toMatchObject({
      status: 'applied',
      revision: 1,
      placement: {
        id: 'placement_0001',
        x_sixteenths: 0,
        stocking_mode: 'tray',
        stocked_unit_count: 12,
        display_width_sixteenths: 175,
        required_depth_sixteenths: 232,
      },
      change_set: { actor: 'webmcp', reason: 'Agent assortment pass', operations: [{ type: 'add_placement', placement: { x_sixteenths: 0, facings_x: 3, facings_z: 4 } }] },
    });
    expect((engine.add_placement_as as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith('version_draft_01', 'jif_creamy_16', 'shelf_01', 0, 'webmcp', 'Agent assortment pass');

    const staleResult = await toolByName(registered, 'planogram.add_product').execute({ product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0 }, { signal });
    expect(staleResult).toMatchObject({ status: 'revision_conflict', expected_revision: 0, current_revision: 1 });

    const unknownArgumentResult = await toolByName(registered, 'planogram.get_product').execute({ product_id: 'jif_creamy_16', x: 1 }, { signal });
    expect(unknownArgumentResult).toMatchObject({ status: 'error', code: 'invalid_input', revision: 1 });

    const undoResult = await toolByName(registered, 'planogram.undo_change_set').execute({ change_set_id: 'change_0001', expected_revision: 1 }, { signal });
    expect(undoResult).toMatchObject({ status: 'applied', revision: 2, change_set: { actor: 'webmcp' } });
    expect(activeContext.placements).toHaveLength(0);
  });

  it('returns a structured cancellation without invoking a command', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);
    const controller = new AbortController();
    controller.abort();
    const result = await toolByName(registered, 'planogram.add_product').execute({ product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0 }, { signal: controller.signal });
    expect(result).toMatchObject({ status: 'error', code: 'cancelled', revision: 0 });
    expect((engine.add_placement_as as unknown as ReturnType<typeof vi.fn>)).not.toHaveBeenCalled();
  });

  it('passes semantic shelf distribution to Rust and rejects unknown layout modes', async () => {
    activeContext = {
      ...activeContext,
      revision: 4,
      placements: [placementView('placement_0001', 'shelf_01', 0), placementView('placement_0002', 'shelf_01', 180)],
    };
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);
    const tool = toolByName(registered, 'planogram.distribute_shelf');

    const invalid = await tool.execute({ shelf_id: 'shelf_01', distribution: 'random', expected_revision: 4 });
    expect(invalid).toMatchObject({ status: 'error', code: 'invalid_input', revision: 4 });
    expect((engine.distribute_shelf_as as unknown as ReturnType<typeof vi.fn>)).not.toHaveBeenCalled();

    const applied = await tool.execute({ shelf_id: 'shelf_01', distribution: 'space_evenly', expected_revision: 4, reason: 'Balance the shelf' });
    expect(applied).toMatchObject({ status: 'applied', revision: 5, affected_ids: ['placement_0001', 'placement_0002'] });
    expect((engine.distribute_shelf_as as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith('version_draft_01', 'shelf_01', 'space_evenly', 4, 'webmcp', 'Balance the shelf');
  });

  it('rejects model-supplied final coordinates in favor of semantic sequence', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);
    const result = await toolByName(registered, 'planogram.preview_changes').execute({
      expected_revision: 0,
      operations: [{ kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', x_sixteenths: 0 }],
    }, { signal: new AbortController().signal });
    expect(result).toMatchObject({ status: 'error', code: 'invalid_input', revision: 0 });
    expect((engine.preview_changes as unknown as ReturnType<typeof vi.fn>)).not.toHaveBeenCalled();
  });

  it('previews generic placement operations without mutation and applies the reviewed proposal atomically', async () => {
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn() });
    await registerPlanogramWebMcp(session, () => undefined);
    const signal = new AbortController().signal;
    const preview = await toolByName(registered, 'planogram.preview_changes').execute({
      expected_revision: 0,
      reason: 'Group the Jif family by size',
      operations: [{ kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 }],
    }, { signal });
    expect(preview).toMatchObject({ status: 'ready', revision: 0, proposal_id: 'proposal_0001', reason: 'Group the Jif family by size', operations: [{ type: 'add_placement', placement: { id: 'placement_0001', x_sixteenths: 0, facings_x: 3, facings_z: 4 } }] });
    expect(activeContext.revision).toBe(0);

    const apply = await toolByName(registered, 'planogram.apply_changes').execute({ proposal_id: 'proposal_0001', expected_revision: 0 }, { signal });
    expect(apply).toMatchObject({ status: 'applied', revision: 1, placements: [{ id: 'placement_0001' }], change_set: { actor: 'webmcp', reason: 'Group the Jif family by size' } });
    const normalizedAdd = { kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 };
    expect((engine.preview_changes as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith('version_draft_01', 0, [normalizedAdd]);
    expect((engine.apply_changes_as as unknown as ReturnType<typeof vi.fn>)).toHaveBeenCalledWith('version_draft_01', 0, [normalizedAdd], 'webmcp', 'Group the Jif family by size');
  });

  it('rejects a pending proposal without changing the draft revision', () => {
    const onProposal = vi.fn();
    const session = new PlanogramSession(engine, { onContext: vi.fn(), onCommand: vi.fn(), onProposal });
    const preview = session.previewChanges({
      versionId: 'version_draft_01',
      expectedRevision: 0,
      reason: 'Try one facing first',
      operations: [{ kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 }],
    });
    expect(preview).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });
    const clearsBeforeReject = (engine.clear_proposal_preview as unknown as ReturnType<typeof vi.fn>).mock.calls.length;
    expect(session.rejectProposal('proposal_0001')).toBe(true);
    expect(engine.clear_proposal_preview).toHaveBeenCalledTimes(clearsBeforeReject + 1);
    expect(onProposal).toHaveBeenLastCalledWith(undefined);
    expect(activeContext.revision).toBe(0);
  });
});
