import {
  addPlacement,
  distributeShelf,
  movePlacement,
  moveShelf,
  removePlacement,
  undoChangeSet,
  type AddPlacementSource,
  type DistributionSource,
  type MoveSource,
  type RemovalSource,
  type UndoSource,
} from './commands';
import type { CommandResult, EngineContext, Placement, PlacementChange, PlanogramValidationResult, PreviewResult, Selection, Shelf, ShelfDistribution, WasmEngine } from './types';

export type SessionSource = MoveSource | AddPlacementSource | DistributionSource | RemovalSource | UndoSource;
export type ProposalApprovalSource = 'human' | 'webmcp';

export interface SessionProposal {
  id: string;
  revision: number;
  reason: string;
  operationCount: number;
  summary: string;
  operations: unknown[];
  impact: {
    shelves: Array<{ shelfId: string; beforePercent: number; afterPercent: number }>;
    minimumGapSixteenths: number;
  };
}

export type SessionPreviewResult =
  | (Extract<PreviewResult, { status: 'ready' }> & { proposal_id: string; reason: string })
  | Exclude<PreviewResult, { status: 'ready' }>;

export interface SessionObservers {
  onContext: (context: EngineContext) => void;
  onCommand: (result: CommandResult, source: SessionSource) => void;
  onProposal?: (proposal: SessionProposal | undefined) => void;
  onProposalApplied?: (result: Extract<CommandResult, { status: 'applied' }>, source: ProposalApprovalSource) => void;
}

/**
 * The live page session is the single browser-side command/query seam. React
 * and WebMCP both use this object, while the Wasm engine remains authoritative
 * for geometry, validation, revisions, and scene patches.
 */
export class PlanogramSession {
  private activeProposal?: { id: string; versionId: string; baseRevision: number; operations: PlacementChange[]; reason: string };
  private nextProposal = 1;

  constructor(
    public readonly engine: WasmEngine,
    private readonly observers: SessionObservers,
  ) {}

  context(): EngineContext {
    return this.engine.context();
  }

  validatePlanogram(): PlanogramValidationResult {
    return this.engine.validate_planogram();
  }

  refresh(): EngineContext {
    const context = this.context();
    this.observers.onContext(context);
    return context;
  }

  private run(source: SessionSource, operation: (context: EngineContext) => CommandResult): CommandResult {
    const result = operation(this.context());
    this.refresh();
    this.observers.onCommand(result, source);
    if (result.status === 'applied') this.clearActiveProposal();
    return result;
  }

  private clearActiveProposal(): void {
    const hadActiveProposal = this.activeProposal !== undefined;
    this.activeProposal = undefined;
    if (hadActiveProposal) {
      this.engine.clear_proposal_preview();
      this.observers.onProposal?.(undefined);
    }
  }

  moveShelf(input: { versionId: string; shelfId: string; elevationSixteenths: number; expectedRevision: number }, source: MoveSource): CommandResult {
    return this.run(source, () => moveShelf(this.engine, input, source));
  }

  movePlacement(input: { versionId: string; placementId: string; targetShelfId: string; xSixteenths: number; expectedRevision: number }, source: MoveSource): CommandResult {
    return this.run(source, () => movePlacement(this.engine, input, source));
  }

  distributeShelf(input: { versionId: string; shelfId: string; distribution: ShelfDistribution; expectedRevision: number; reason?: string }, source: DistributionSource): CommandResult {
    return this.run(source, () => distributeShelf(this.engine, input, source));
  }

  addPlacement(input: { versionId: string; productId: string; shelfId: string; expectedRevision: number; reason?: string }, source: AddPlacementSource): CommandResult {
    return this.run(source, () => addPlacement(this.engine, input, source));
  }

  removePlacement(input: { versionId: string; placementId: string; expectedRevision: number }, source: RemovalSource): CommandResult {
    return this.run(source, () => removePlacement(this.engine, input, source));
  }

  undoChangeSet(input: { versionId: string; changeSetId: string; expectedRevision: number }, source: UndoSource): CommandResult {
    return this.run(source, () => undoChangeSet(this.engine, input, source));
  }

  previewChanges(input: { versionId: string; expectedRevision: number; operations: PlacementChange[]; reason?: string }): SessionPreviewResult {
    this.clearActiveProposal();
    const result = this.engine.preview_changes(input.versionId, input.expectedRevision, input.operations);
    if (result.status !== 'ready') return result;
    const proposalId = `proposal_${String(this.nextProposal++).padStart(4, '0')}`;
    const reason = input.reason?.trim() || 'WebMCP placement proposal';
    this.activeProposal = { id: proposalId, versionId: input.versionId, baseRevision: result.revision, operations: input.operations, reason };
    const summary = proposalSummary(result.operations);
    const impact = proposalImpact(this.context(), result);
    this.observers.onProposal?.({ id: proposalId, revision: result.revision, reason, operationCount: result.operations.length, summary, operations: result.operations, impact });
    return { ...result, proposal_id: proposalId, reason };
  }

  applyChanges(input: { versionId: string; proposalId: string; expectedRevision: number }, source: ProposalApprovalSource): CommandResult {
    const proposal = this.activeProposal;
    if (!proposal || proposal.id !== input.proposalId) {
      return { status: 'not_found', entity: 'proposal', id: input.proposalId };
    }
    if (proposal.versionId !== input.versionId) {
      return { status: 'not_found', entity: 'proposal', id: input.proposalId };
    }
    if (input.expectedRevision !== proposal.baseRevision) {
      return { status: 'revision_conflict', expected_revision: input.expectedRevision, current_revision: this.context().revision };
    }
    const result = this.run(source, () => this.engine.apply_changes_as(
      input.versionId,
      input.expectedRevision,
      proposal.operations,
      source,
      proposal.reason,
    ));
    if (result.status === 'applied') {
      this.observers.onProposalApplied?.(result, source);
    }
    return result;
  }

  rejectProposal(proposalId: string): boolean {
    if (this.activeProposal?.id !== proposalId) return false;
    this.clearActiveProposal();
    return true;
  }

  select(selection?: Selection): void {
    if (!selection) this.engine.clear_selection();
    else if (selection.kind === 'shelf') this.engine.select_shelf(selection.id);
    else this.engine.select_placement(selection.id);
  }

  findShelf(shelfId: string): Shelf | undefined {
    return this.context().fixture.sections.flatMap(section => section.shelves).find(shelf => shelf.id === shelfId);
  }

  findPlacement(placementId: string): Placement | undefined {
    return this.context().placements.find(placement => placement.id === placementId);
  }
}

function proposalSummary(operations: unknown[]): string {
  const counts = operations.reduce<{ add: number; move: number; remove: number }>((summary, operation) => {
    if (!operation || typeof operation !== 'object') return summary;
    const type = 'type' in operation ? String(operation.type).replace('_placement', '') : 'kind' in operation ? String(operation.kind) : '';
    if (type === 'add' || type === 'move' || type === 'remove') summary[type] += 1;
    return summary;
  }, { add: 0, move: 0, remove: 0 });
  return (['add', 'move', 'remove'] as const)
    .map(kind => `${counts[kind]} ${kind === 'add' ? 'addition' : kind === 'remove' ? 'removal' : kind}${counts[kind] === 1 ? '' : 's'}`)
    .join(' · ');
}

function proposalImpact(context: EngineContext, result: Extract<PreviewResult, { status: 'ready' }>): SessionProposal['impact'] {
  const affectedPlacementIds = new Set(result.affected_ids);
  const currentAffected = context.placements.filter(placement => affectedPlacementIds.has(placement.id));
  const proposedAffected = result.preview_scene.placements.filter(placement => affectedPlacementIds.has(placement.id));
  const affectedShelfIds = new Set([
    ...currentAffected.map(placement => placement.shelf_id),
    ...proposedAffected.map(placement => placement.shelf_id),
  ]);
  const shelves = context.fixture.sections
    .flatMap(section => section.shelves)
    .filter(shelf => affectedShelfIds.has(shelf.id))
    .map(shelf => {
      const currentWidths = context.placements
        .filter(placement => placement.shelf_id === shelf.id)
        .map(placement => placement.geometry.display_width);
      const proposedWidths = result.preview_scene.placements
        .filter(placement => placement.shelf_id === shelf.id)
        .map(placement => placement.width);
      const shelfUtilization = (widths: number[]) => Math.min(100, Math.round((widths.reduce((sum, width) => sum + width, 0) + Math.max(0, widths.length - 1) * 2) / shelf.width * 100));
      return { shelfId: shelf.id, beforePercent: shelfUtilization(currentWidths), afterPercent: shelfUtilization(proposedWidths) };
    });
  return { shelves, minimumGapSixteenths: 2 };
}
