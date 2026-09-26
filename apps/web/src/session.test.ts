import { describe, expect, it, vi } from 'vitest';
import { PlanogramSession, type ProposalApprovalSource, type SessionObservers } from './session';
import { VERSION_ID, adjustableShelf, appliedResult, makeContext, readyPreview, reflowOperation, revisionConflict, trayPlacement } from './testFixtures';
import type { CommandResult, EngineContext, PreviewResult, WasmEngine } from './types';

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
    preview_changes: vi.fn((): PreviewResult => readyPreview(context.revision)),
    preview_shelf_allocation: vi.fn((): PreviewResult => readyPreview(context.revision, [reflowOperation(trayPlacement({ facings_x: 1, facings_z: 1 }), { x: 4, facings_x: 4 })])),
    clear_proposal_preview: vi.fn(),
    apply_changes_as: vi.fn((_versionId: string, expectedRevision: number, _operations: unknown[], actor: string, reason: string) => commit(expectedRevision, actor, reason)),
    apply_shelf_allocation_as: vi.fn((_versionId: string, _shelfId: string, _strategy: string, expectedRevision: number, actor: string, reason: string) => commit(expectedRevision, actor, reason, ['placement_0001'])),
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
