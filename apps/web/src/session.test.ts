import { describe, expect, it, vi } from 'vitest';
import { PlanogramSession, type ProposalApprovalSource, type SessionObservers } from './session';
import { BAY_FILE_TEXT, VERSION_ID, adjustableShelf, appliedResult, makeContext, readyPreview, readySalesAllocationPreview, reflowOperation, revisionConflict, salesAllocationRequest, trayPlacement } from './testFixtures';
import type { CommandResult, EngineContext, PreviewResult, SalesAllocationRequest, WasmEngine } from './types';

const operation = { kind: 'add' as const, product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 };

function makeHarness(startingContext: EngineContext = makeContext()) {
  let context = startingContext;
  const commit = (expectedRevision: number, actor: string, reason: string, affectedIds: string[] = []): CommandResult => {
    if (expectedRevision !== context.revision) return revisionConflict(expectedRevision, context.revision);
    const result = appliedResult(context.revision + 1, { actor, reason, operations: [{ type: 'add_placement' }], affectedIds });
    context = { ...context, revision: result.revision, latest_change_set_id: result.change_set.id };
    return result;
  };
  const mocks = {
    context: vi.fn(() => context),
    start_cereal: vi.fn(),
    select_alternative: vi.fn(),
    duplicate_alternative: vi.fn(),
    export_bay: vi.fn(() => BAY_FILE_TEXT),
    inspect_bay: vi.fn(() => 'Peanut butter bay'),
    restore_bay: vi.fn(() => { context = makeContext(); return 'Peanut butter bay'; }),
    preview_changes: vi.fn((): PreviewResult => readyPreview(context.revision)),
    preview_shelf_allocation: vi.fn((): PreviewResult => readyPreview(context.revision, [reflowOperation(trayPlacement({ facings_x: 1, facings_z: 1 }), { x: 4, facings_x: 4 })])),
    preview_sales_allocation: vi.fn((_versionId: string, request: SalesAllocationRequest): PreviewResult => readySalesAllocationPreview(context.revision, request)),
    clear_proposal_preview: vi.fn(),
    apply_changes_as: vi.fn((_versionId: string, expectedRevision: number, _operations: unknown[], actor: string, reason: string) => commit(expectedRevision, actor, reason)),
    apply_shelf_allocation_as: vi.fn((_versionId: string, _shelfId: string, _strategy: string, expectedRevision: number, actor: string, reason: string) => commit(expectedRevision, actor, reason, ['placement_0001'])),
    apply_sales_allocation_as: vi.fn((_versionId: string, _request: SalesAllocationRequest, expectedRevision: number, actor: string, reason: string) => commit(expectedRevision, actor, reason, ['placement_0002'])),
    add_placement_as: vi.fn((_versionId: string, _productId: string, _shelfId: string, expectedRevision: number, actor: string, reason: string) => commit(expectedRevision, actor, reason)),
  };
  const observers = {
    onContext: vi.fn(),
    onCommand: vi.fn(),
    onProposal: vi.fn(),
    onProposalApplied: vi.fn(),
  } satisfies SessionObservers;
  const session = new PlanogramSession(mocks as unknown as WasmEngine, observers);
  return { session, observers, ...mocks };
}

function preview(session: PlanogramSession, reason: string) {
  return session.previewChanges({ versionId: VERSION_ID, expectedRevision: session.context().revision, operations: [operation], reason });
}

function apply(session: PlanogramSession, proposalId: string, expectedRevision = 0, source: ProposalApprovalSource = 'human') {
  return session.applyChanges({ versionId: VERSION_ID, proposalId, expectedRevision }, source);
}

describe('PlanogramSession proposal lifecycle', () => {
  it('keeps shelf allocation semantic through preview and apply', () => {
    const harness = makeHarness();

    const proposal = harness.session.previewShelfAllocation({
      versionId: VERSION_ID,
      shelfId: 'shelf_01',
      strategy: 'fill_evenly',
      expectedRevision: 0,
      reason: 'Fill the shelf evenly',
    });
    expect(proposal).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001', operations: [{ type: 'reflow_placement' }] });
    expect(harness.preview_shelf_allocation).toHaveBeenCalledWith(VERSION_ID, 'shelf_01', 'fill_evenly', 0);

    expect(apply(harness.session, 'proposal_0001', 0, 'webmcp')).toMatchObject({ status: 'applied', revision: 1 });
    expect(harness.apply_shelf_allocation_as).toHaveBeenCalledWith(VERSION_ID, 'shelf_01', 'fill_evenly', 0, 'webmcp', 'Fill the shelf evenly');
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
  });

  it.each(['human', 'webmcp'] as const)('records the actual %s approval actor', (source: ProposalApprovalSource) => {
    const harness = makeHarness();
    expect(preview(harness.session, 'Balance the assortment')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });

    const result = apply(harness.session, 'proposal_0001', 0, source);

    expect(result).toMatchObject({ status: 'applied', change_set: { actor: source, reason: 'Balance the assortment' } });
    expect(harness.apply_changes_as).toHaveBeenCalledWith(VERSION_ID, 0, [operation], source, 'Balance the assortment');
    expect(harness.observers.onProposalApplied).toHaveBeenCalledWith(expect.objectContaining({ change_set: expect.objectContaining({ actor: source }) }), source);
  });

  it('keeps only the latest ready proposal active', () => {
    const harness = makeHarness();
    expect(preview(harness.session, 'First idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });
    expect(preview(harness.session, 'Replacement idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0002' });
    expect(harness.clear_proposal_preview).toHaveBeenCalledOnce();

    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found', entity: 'proposal' });
    expect(apply(harness.session, 'proposal_0002')).toMatchObject({ status: 'applied' });
  });

  it('clears the prior proposal before an invalid replacement preview', () => {
    const harness = makeHarness();
    harness.preview_changes
      .mockReturnValueOnce(readyPreview(0))
      .mockReturnValueOnce({ status: 'validation_failed', revision: 0, validation: { issues: [{ code: 'invalid', message: 'Does not fit' }] } });
    expect(preview(harness.session, 'Valid idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });

    expect(preview(harness.session, 'Invalid replacement')).toMatchObject({ status: 'validation_failed' });
    expect(harness.clear_proposal_preview).toHaveBeenCalledOnce();
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(undefined);
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found', entity: 'proposal' });
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
  });

  it('rejects a pending proposal without changing the draft revision', () => {
    const harness = makeHarness();
    expect(preview(harness.session, 'Try one facing first')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });

    expect(harness.session.rejectProposal('proposal_0001')).toBe(true);
    expect(harness.clear_proposal_preview).toHaveBeenCalledOnce();
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(undefined);
    expect(harness.session.context().revision).toBe(0);
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found', entity: 'proposal' });
  });

  it('requires the active proposal base revision when applying', () => {
    const harness = makeHarness(makeContext({ revision: 4 }));
    expect(preview(harness.session, 'Revision-safe idea')).toMatchObject({ status: 'ready', revision: 4, proposal_id: 'proposal_0001' });

    expect(apply(harness.session, 'proposal_0001', 5)).toEqual(revisionConflict(5, 4));
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
    expect(apply(harness.session, 'proposal_0001', 4)).toMatchObject({ status: 'applied' });
  });

  it('reports utilization for every source and target shelf in a cross-shelf preview', () => {
    const placement = trayPlacement({ geometry: { display_width: 100, display_height: 80, required_depth: 200 } });
    const harness = makeHarness(makeContext({
      shelves: [adjustableShelf('shelf_01', 192), adjustableShelf('shelf_02', 384)],
      placements: [placement],
    }));
    const moved = readyPreview(0, [{ type: 'move_placement', placement_id: placement.id }]);
    moved.affected_ids = [placement.id];
    moved.preview_scene.placements = [{
      id: placement.id,
      product_id: placement.product_id,
      shelf_id: 'shelf_02',
      x: 0,
      width: 100,
      height: 80,
      required_depth: 200,
      stocking_mode: 'tray',
      stocked_unit_count: 12,
      facings_x: 3,
      facings_y: 1,
      facings_z: 4,
      tray_front_lip_height: 20,
      package_shape: 'jar',
      color: [1, 2, 3],
      lid_color: [4, 5, 6],
    }];
    harness.preview_changes.mockReturnValue(moved);

    expect(preview(harness.session, 'Move the tray')).toMatchObject({ status: 'ready' });
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(expect.objectContaining({
      impact: {
        shelves: [
          { shelfId: 'shelf_01', beforePercent: 13, afterPercent: 0 },
          { shelfId: 'shelf_02', beforePercent: 0, afterPercent: 13 },
        ],
        minimumGapSixteenths: 2,
      },
    }));
  });
});


describe('PlanogramSession bay files', () => {
  it('exports only the engine snapshot while preserving a pending proposal', () => {
    const harness = makeHarness();
    preview(harness.session, 'Pending idea');
    expect(harness.session.exportBay('Peanut butter bay')).toBe(BAY_FILE_TEXT);
    expect(harness.export_bay).toHaveBeenCalledWith('Peanut butter bay');
    expect(harness.session.hasPendingProposal()).toBe(true);
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
    expect(harness.clear_proposal_preview).not.toHaveBeenCalled();
  });

  it('refreshes the mirror and discards proposal state only after a successful restore', () => {
    const harness = makeHarness(makeContext({ revision: 4 }));
    preview(harness.session, 'Pending idea');
    expect(harness.session.inspectBay(BAY_FILE_TEXT)).toBe('Peanut butter bay');
    expect(harness.session.hasPendingProposal()).toBe(true);
    harness.restore_bay.mockImplementationOnce(() => { throw new Error('Unsupported bay file'); });
    expect(() => harness.session.restoreBay(BAY_FILE_TEXT, 4)).toThrow('Unsupported');
    expect(harness.session.hasPendingProposal()).toBe(true);
    expect(harness.clear_proposal_preview).not.toHaveBeenCalled();
    expect(harness.session.context().revision).toBe(4);
    expect(harness.session.restoreBay(BAY_FILE_TEXT, 4)).toBe('Peanut butter bay');
    expect(harness.restore_bay).toHaveBeenLastCalledWith(BAY_FILE_TEXT, 4);
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(harness.observers.onContext).toHaveBeenLastCalledWith(makeContext());
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(undefined);
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found' });
  });
});

describe('PlanogramSession sales allocation', () => {
  function previewSales(harness: ReturnType<typeof makeHarness>, request = salesAllocationRequest()) {
    return harness.session.previewSalesAllocation({ versionId: VERSION_ID, request, expectedRevision: 0, reason: 'Match the synthetic sales mix' });
  }

  it.each(['human', 'webmcp'] as const)('retains cloned semantic intent and records the %s approval actor', source => {
    const harness = makeHarness();
    const request = salesAllocationRequest();
    const original = structuredClone(request);
    const result = previewSales(harness, request);
    expect(result).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001', sales_allocation: { basis: 'revenue', target: 'facings' } });
    expect(harness.session.context().revision).toBe(0);
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(expect.objectContaining({
      salesAllocation: readySalesAllocationPreview(0).sales_allocation,
      summary: '0 additions · 0 moves · 1 facing update · 0 removals',
    }));
    request.scope = { kind: 'bay', section_id: 'another_section' };
    request.basis = 'units';
    request.max_facings = 100;

    expect(apply(harness.session, 'proposal_0001', 0, source)).toMatchObject({ status: 'applied', revision: 1, change_set: { actor: source } });
    expect(harness.apply_sales_allocation_as).toHaveBeenCalledWith(VERSION_ID, original, 0, source, 'Match the synthetic sales mix');
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
    expect(harness.apply_shelf_allocation_as).not.toHaveBeenCalled();
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(apply(harness.session, 'proposal_0001', 1, source)).toMatchObject({ status: 'not_found' });
    expect(harness.apply_sales_allocation_as).toHaveBeenCalledOnce();
  });

  it('also clones the nested scope so its shelf cannot change after preview', () => {
    const harness = makeHarness();
    const request = salesAllocationRequest();
    previewSales(harness, request);
    if (request.scope.kind === 'shelf') request.scope.shelf_id = 'shelf_02';
    apply(harness.session, 'proposal_0001');
    expect(harness.apply_sales_allocation_as).toHaveBeenCalledWith(VERSION_ID, salesAllocationRequest(), 0, 'human', 'Match the synthetic sales mix');
  });

  it('discards sales previews on cancellation, replacement and committed edits', () => {
    const harness = makeHarness();
    previewSales(harness);
    expect(harness.session.rejectProposal('proposal_0001')).toBe(true);
    expect(harness.session.context().revision).toBe(0);
    expect(harness.session.rejectProposal('proposal_0001')).toBe(false);
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found' });

    previewSales(harness);
    preview(harness.session, 'Replacement');
    expect(apply(harness.session, 'proposal_0002')).toMatchObject({ status: 'not_found' });
    previewSales(harness);
    harness.session.addPlacement({ versionId: VERSION_ID, productId: 'jif_creamy_16', shelfId: 'shelf_01', expectedRevision: 0 }, 'webmcp');
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(apply(harness.session, 'proposal_0004', 1)).toMatchObject({ status: 'not_found' });
    expect(harness.apply_sales_allocation_as).not.toHaveBeenCalled();
  });

  it.each([
    { status: 'validation_failed', revision: 0, validation: { issues: [{ code: 'sales_data', message: 'No positive synthetic contribution.' }] } },
    { status: 'invalid_command', message: 'Infeasible facing bounds.' },
    { status: 'forbidden', message: 'Baseline is immutable.' },
  ] satisfies PreviewResult[])('does not retain a rejected preview: $status', rejection => {
    const harness = makeHarness();
    previewSales(harness);
    harness.preview_sales_allocation.mockReturnValueOnce(rejection);
    expect(previewSales(harness)).toEqual(rejection);
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(harness.session.context().revision).toBe(0);
    expect(harness.apply_sales_allocation_as).not.toHaveBeenCalled();
  });

  it.each([
    { changed: { version_id: 'cereal_alternative_2' }, status: 'not_found' },
    { changed: { document_revision: 1 }, status: 'invalid_command' },
    { changed: { revision: 1 }, status: 'revision_conflict' },
  ])('blocks stale approval when document context changes ($status)', ({ changed, status }) => {
    const harness = makeHarness();
    previewSales(harness);
    harness.context.mockReturnValue(makeContext(changed));
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status });
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(harness.apply_sales_allocation_as).not.toHaveBeenCalled();
  });

  it('exports committed state without approving sales allocation and invalidates it on successful open', () => {
    const harness = makeHarness();
    previewSales(harness);
    expect(harness.session.exportBay('Sales allocation')).toBe(BAY_FILE_TEXT);
    expect(harness.session.hasPendingProposal()).toBe(true);
    expect(harness.apply_sales_allocation_as).not.toHaveBeenCalled();
    harness.session.restoreBay(BAY_FILE_TEXT, 0);
    expect(harness.session.hasPendingProposal()).toBe(false);
    expect(apply(harness.session, 'proposal_0001')).toMatchObject({ status: 'not_found' });
  });
});

it('routes scenario operations with document revision and clears proposals only after success',()=>{
 const h=makeHarness();h.context.mockReturnValue({...makeContext(),document_revision:42});
 preview(h.session,'Pending');h.select_alternative.mockImplementationOnce(()=>{throw new Error('stale');});
 expect(()=>h.session.selectAlternative(1)).toThrow('stale');expect(h.session.hasPendingProposal()).toBe(true);
 h.session.selectAlternative(1);expect(h.select_alternative).toHaveBeenLastCalledWith(1,42);expect(h.session.hasPendingProposal()).toBe(false);
 h.session.duplicateAlternative();expect(h.duplicate_alternative).toHaveBeenCalledWith(42);
 h.session.startCereal(123,42);expect(h.start_cereal).toHaveBeenCalledWith(123,42);
});
