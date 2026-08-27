import { describe, expect, it, vi } from 'vitest';
import { PlanogramSession, type ProposalApprovalSource, type SessionObservers } from './session';
import type { CommandResult, EngineContext, PreviewResult, WasmEngine } from './types';

const operation = { kind: 'add' as const, product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 };

function makeContext(revision = 0): EngineContext {
  return {
    version_id: 'version_draft_01',
    version_status: 'draft',
    revision,
    fixture: {
      id: 'fixture_standard_4ft',
      name: "4' Standard Bay",
      width: 768,
      height: 1344,
      depth: 352,
      sections: [],
    },
    products: [],
    placements: [],
  };
}

function readyPreview(revision: number): Extract<PreviewResult, { status: 'ready' }> {
  return {
    status: 'ready',
    revision,
    operations: [{ type: 'add_placement' }],
    affected_ids: [],
    validation: { issues: [] },
    preview_scene: { revision, fixture_id: 'fixture_standard_4ft', width: 768, height: 1344, shelves: [], placements: [] },
  };
}

function makeHarness(startingRevision = 0, startingContext?: EngineContext) {
  let context = startingContext ?? makeContext(startingRevision);
  const preview_changes = vi.fn((): PreviewResult => readyPreview(context.revision));
  const clear_proposal_preview = vi.fn();
  const apply_changes_as = vi.fn((_versionId: string, expectedRevision: number, _operations: unknown[], actor: string, reason: string): CommandResult => {
    if (expectedRevision !== context.revision) {
      return { status: 'revision_conflict', expected_revision: expectedRevision, current_revision: context.revision };
    }
    const revision = context.revision + 1;
    const changeSetId = `change_${String(revision).padStart(4, '0')}`;
    context = { ...context, revision, latest_change_set_id: changeSetId };
    return {
      status: 'applied',
      revision,
      change_set: { id: changeSetId, actor, reason, base_revision: revision - 1, resulting_revision: revision, operations: [{ type: 'add_placement' }] },
      affected_ids: [],
      validation: { issues: [] },
      scene_patch: { revision, shelves: [], placements: [], removed_placement_ids: [] },
    };
  });
  const engine = {
    context: vi.fn(() => context),
    preview_changes,
    clear_proposal_preview,
    apply_changes_as,
  } as unknown as WasmEngine;
  const observers: SessionObservers = {
    onContext: vi.fn(),
    onCommand: vi.fn(),
    onProposal: vi.fn(),
    onProposalApplied: vi.fn(),
  };
  return { engine, observers, preview_changes, clear_proposal_preview, apply_changes_as };
}

function preview(session: PlanogramSession, reason: string) {
  return session.previewChanges({ versionId: 'version_draft_01', expectedRevision: session.context().revision, operations: [operation], reason });
}

describe('PlanogramSession proposal lifecycle', () => {
  it.each(['human', 'webmcp'] as const)('records the actual %s approval actor', (source: ProposalApprovalSource) => {
    const harness = makeHarness();
    const session = new PlanogramSession(harness.engine, harness.observers);
    expect(preview(session, 'Balance the assortment')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });

    const result = session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0001', expectedRevision: 0 }, source);

    expect(result).toMatchObject({ status: 'applied', change_set: { actor: source, reason: 'Balance the assortment' } });
    expect(harness.apply_changes_as).toHaveBeenCalledWith('version_draft_01', 0, [operation], source, 'Balance the assortment');
    expect(harness.observers.onProposalApplied).toHaveBeenCalledWith(expect.objectContaining({ change_set: expect.objectContaining({ actor: source }) }), source);
  });

  it('keeps only the latest ready proposal active', () => {
    const harness = makeHarness();
    const session = new PlanogramSession(harness.engine, harness.observers);
    expect(preview(session, 'First idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });
    expect(preview(session, 'Replacement idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0002' });
    expect(harness.clear_proposal_preview).toHaveBeenCalledOnce();

    expect(session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0001', expectedRevision: 0 }, 'human')).toMatchObject({ status: 'not_found', entity: 'proposal' });
    expect(session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0002', expectedRevision: 0 }, 'human')).toMatchObject({ status: 'applied' });
  });

  it('clears the prior proposal before an invalid replacement preview', () => {
    const harness = makeHarness();
    harness.preview_changes
      .mockReturnValueOnce(readyPreview(0))
      .mockReturnValueOnce({ status: 'validation_failed', revision: 0, validation: { issues: [{ code: 'invalid', message: 'Does not fit' }] } });
    const session = new PlanogramSession(harness.engine, harness.observers);
    expect(preview(session, 'Valid idea')).toMatchObject({ status: 'ready', proposal_id: 'proposal_0001' });

    expect(preview(session, 'Invalid replacement')).toMatchObject({ status: 'validation_failed' });
    expect(harness.clear_proposal_preview).toHaveBeenCalledOnce();
    expect(harness.observers.onProposal).toHaveBeenLastCalledWith(undefined);
    expect(session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0001', expectedRevision: 0 }, 'human')).toMatchObject({ status: 'not_found', entity: 'proposal' });
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
  });

  it('requires the active proposal base revision when applying', () => {
    const harness = makeHarness(4);
    const session = new PlanogramSession(harness.engine, harness.observers);
    expect(preview(session, 'Revision-safe idea')).toMatchObject({ status: 'ready', revision: 4, proposal_id: 'proposal_0001' });

    expect(session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0001', expectedRevision: 5 }, 'human')).toEqual({ status: 'revision_conflict', expected_revision: 5, current_revision: 4 });
    expect(harness.apply_changes_as).not.toHaveBeenCalled();
    expect(session.applyChanges({ versionId: 'version_draft_01', proposalId: 'proposal_0001', expectedRevision: 4 }, 'human')).toMatchObject({ status: 'applied' });
  });

  it('reports utilization for every source and target shelf in a cross-shelf preview', () => {
    const context = makeContext();
    context.fixture.sections = [{
      id: 'section_01',
      fixture_id: context.fixture.id,
      sequence: 0,
      width: 768,
      height: 1344,
      shelves: [
        { id: 'shelf_01', section_id: 'section_01', kind: 'adjustable', width: 768, depth: 256, elevation: 192 },
        { id: 'shelf_02', section_id: 'section_01', kind: 'adjustable', width: 768, depth: 256, elevation: 384 },
      ],
    }];
    context.placements = [{
      id: 'placement_0001',
      product_id: 'jif_creamy_16',
      shelf_id: 'shelf_01',
      x: 0,
      stocking_mode: 'tray',
      facings_x: 3,
      facings_y: 1,
      facings_z: 4,
      stocked_unit_count: 12,
      geometry: { display_width: 100, display_height: 80, required_depth: 200 },
      tray_front_lip_height: 20,
    }];
    const harness = makeHarness(0, context);
    harness.preview_changes.mockReturnValue({
      status: 'ready',
      revision: 0,
      operations: [{ type: 'move_placement', placement_id: 'placement_0001' }],
      affected_ids: ['placement_0001'],
      validation: { issues: [] },
      preview_scene: {
        revision: 0,
        fixture_id: context.fixture.id,
        width: 768,
        height: 1344,
        shelves: [],
        placements: [{
          id: 'placement_0001',
          product_id: 'jif_creamy_16',
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
        }],
      },
    });
    const session = new PlanogramSession(harness.engine, harness.observers);

    expect(preview(session, 'Move the tray')).toMatchObject({ status: 'ready' });
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
