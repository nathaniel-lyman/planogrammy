import { useCallback, useEffect, useMemo, useRef, useState, type Ref } from 'react';
import { CheckCircle2, Focus, Minus, Package, PackagePlus, Plus, RotateCcw, Search, Sparkles, Trash2, Undo2, X, XCircle } from 'lucide-react';
import { resultError, type MoveSource, type PlacementSource, type RemovalSource } from './commands';
import { formatImperial, parseImperial } from './imperial';
import { searchProducts } from './queries';
import { PlanogramSession, type ProposalApprovalSource, type SessionProposal } from './session';
import { useUiStore } from './store';
import { registerPlanogramWebMcp } from './webmcp';
import type { ChangeSet, CommandResult, Placement, Product, Selection, Shelf, ShelfDistribution, StockingMode, WasmEngine } from './types';

const CANVAS_ID = 'planogram-canvas';
const SHELF_READY_TRAY_LABEL = 'Shelf-ready tray';

function allShelves(context: NonNullable<ReturnType<typeof useUiStore.getState>['context']>): Shelf[] {
  return context.fixture.sections.flatMap(section => section.shelves);
}

function statusError(result: CommandResult): string | undefined { return result.status === 'applied' ? undefined : resultError(result); }

function productTooltip(product: Product) {
  const stocking = product.tray ? `${SHELF_READY_TRAY_LABEL}, ${product.tray.facings_x} facings × ${product.tray.units_deep} deep` : 'Loose stocked';
  return `${product.brand} ${product.description} · ${product.size_oz} · ${formatImperial(product.dimensions.width)} W × ${formatImperial(product.dimensions.height)} H × ${formatImperial(product.dimensions.depth)} D · ${stocking}`;
}

const compactNumber = new Intl.NumberFormat('en-US', { maximumFractionDigits: 2 });
const currency = new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', minimumFractionDigits: 2, maximumFractionDigits: 2 });

function formatNetWeight(product: Product) {
  return `${compactNumber.format(product.net_weight_ounces_hundredths / 100)} oz`;
}

function formatSalesPerStoreWeek(product: Product) {
  return currency.format(product.performance.sales_per_store_per_week_cents / 100);
}

function formatUnitsPerStoreWeek(product: Product) {
  return compactNumber.format(product.performance.units_per_store_per_week_milliunits / 1_000);
}

function formatGrossMargin(product: Product) {
  return `${compactNumber.format(product.performance.gross_margin_basis_points / 100)}%`;
}

function placementStockingLabel(placement: Placement) {
  return placement.stocking_mode === 'tray'
    ? `${SHELF_READY_TRAY_LABEL} · ${placement.facings_x} facings × ${placement.facings_z} deep · ${placement.stocked_unit_count} units`
    : `Loose · ${placement.facings_x} × ${placement.facings_y} × ${placement.facings_z} facings · ${placement.stocked_unit_count} units`;
}

function productAccessibilitySummary(product: Product) {
  const tray = product.tray
    ? ` ${SHELF_READY_TRAY_LABEL}: ${product.tray.facings_x} facings by ${product.tray.units_deep} units deep; loaded envelope ${formatImperial(product.tray.outer_width_sixteenths)} wide by ${formatImperial(product.tray.outer_height_sixteenths)} high by ${formatImperial(product.tray.outer_depth_sixteenths)} deep; front lip ${formatImperial(product.tray.front_lip_height_sixteenths)}.`
    : ' Loose stocked.';
  return `Net weight ${formatNetWeight(product)}. Unit dimensions ${formatImperial(product.dimensions.width)} wide by ${formatImperial(product.dimensions.height)} high by ${formatImperial(product.dimensions.depth)} deep. Casepack ${product.casepack_quantity}. Sales ${formatSalesPerStoreWeek(product)} per store per week. Units ${formatUnitsPerStoreWeek(product)} per store per week. Gross margin ${formatGrossMargin(product)}. ${product.performance.period}. ${product.performance.source}.${tray}`;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function proposalProductName(productId: unknown, products: Product[], fallback = 'Product') {
  if (typeof productId !== 'string') return fallback;
  const product = products.find(candidate => candidate.id === productId);
  return product ? `${product.brand} ${product.description} (${product.size_oz})` : productId;
}

function shelfLabel(shelfId: unknown) {
  return typeof shelfId === 'string' ? shelfId.replace('shelf_', 'Shelf ').replace('_', ' ') : 'shelf';
}

function proposalOperationLabel(operation: unknown, products: Product[], placements: Placement[]) {
  if (!isRecord(operation) || typeof operation.type !== 'string') return 'Unrecognized placement operation';
  const placement = isRecord(operation.placement) ? operation.placement : undefined;
  if (operation.type === 'add_placement' && placement) {
    return `Add ${proposalProductName(placement.product_id, products)} to ${shelfLabel(placement.shelf_id)} at ${formatImperial(typeof placement.x === 'number' ? placement.x : 0)}`;
  }
  if (operation.type === 'remove_placement' && placement) {
    return `Remove ${proposalProductName(placement.product_id, products)} from ${shelfLabel(placement.shelf_id)} at ${formatImperial(typeof placement.x === 'number' ? placement.x : 0)}`;
  }
  if (operation.type === 'move_placement') {
    const before = isRecord(operation.before) ? operation.before : undefined;
    const after = isRecord(operation.after) ? operation.after : undefined;
    const existing = placements.find(candidate => candidate.id === operation.placement_id);
    const label = existing ? proposalProductName(existing.product_id, products, existing.id) : String(operation.placement_id ?? 'placement');
    if (before && after) {
      return `Move ${label} from ${shelfLabel(before.shelf_id)} at ${formatImperial(typeof before.x === 'number' ? before.x : 0)} to ${shelfLabel(after.shelf_id)} at ${formatImperial(typeof after.x === 'number' ? after.x : 0)}`;
    }
    return `Move ${label}`;
  }
  return `Apply ${operation.type.replaceAll('_', ' ')}`;
}

function proposalOperationMeta(operation: unknown, products: Product[]) {
  if (!isRecord(operation) || typeof operation.type !== 'string') return undefined;
  const placement = isRecord(operation.placement) ? operation.placement : undefined;
  if ((operation.type === 'add_placement' || operation.type === 'remove_placement') && placement) {
    const x = typeof placement.x === 'number' ? placement.x : 0;
    const facings = typeof placement.facings_x === 'number' ? placement.facings_x : 1;
    const facingsZ = typeof placement.facings_z === 'number' ? placement.facings_z : 1;
    const product = products.find(candidate => candidate.id === placement.product_id);
    const stocking = product?.tray
      ? `${SHELF_READY_TRAY_LABEL} · ${facings} facings × ${facingsZ} deep`
      : `${facings} ${facings === 1 ? 'facing' : 'facings'}`;
    return `Position ${formatImperial(x)} · ${stocking}`;
  }
  if (operation.type === 'move_placement' && isRecord(operation.after)) {
    return `New position ${formatImperial(typeof operation.after.x === 'number' ? operation.after.x : 0)}`;
  }
}

function ProductSwatch({ product }: { product: Product }) {
  const [red, green, blue] = product.color;
  return <span
    className="product-swatch"
    style={{ backgroundColor: `rgb(${red} ${green} ${blue})` }}
    aria-hidden="true"
    title={productTooltip(product)}
  />;
}

function ProposalReview({
  proposal,
  products,
  placements,
  revisionRequested,
  onAccept,
  onRevise,
  onReject,
}: {
  proposal: SessionProposal;
  products: Product[];
  placements: Placement[];
  revisionRequested: boolean;
  onAccept: () => void;
  onRevise: () => void;
  onReject: () => void;
}) {
  return <aside className="proposal-review" aria-labelledby="proposal-review-heading">
    <div className="proposal-review-header">
      <div><span className="section-label">Pending change set</span><h1 id="proposal-review-heading">Proposal review</h1></div>
      <button className="proposal-close" type="button" onClick={onReject} aria-label="Dismiss proposal"><X size={18}/></button>
    </div>

    <section className="proposal-reason">
      <div className="proposal-section-title"><Sparkles size={15}/><span>Reason</span></div>
      <p>{proposal.reason}</p>
    </section>

    <section className="proposal-summary">
      <span className="section-label">Change summary</span>
      <strong>{proposal.summary}</strong>
      <div className="constraint-pass" role="status"><CheckCircle2 size={17}/><span>All constraints pass</span></div>
    </section>

    <section className="proposal-impact">
      <span className="section-label">Impact</span>
      <dl>
        {proposal.impact.shelves.map(shelf => <div key={shelf.shelfId}><dt>{shelfLabel(shelf.shelfId)} utilization</dt><dd><span>{shelf.beforePercent}%</span><strong>→</strong><span>{shelf.afterPercent}%</span></dd></div>)}
        {proposal.impact.shelves.length === 0 && <div><dt>Shelf utilization</dt><dd>Unchanged</dd></div>}
        <div><dt>Minimum gap</dt><dd>{formatImperial(proposal.impact.minimumGapSixteenths)}</dd></div>
      </dl>
    </section>

    <section className="proposal-operations">
      <div className="proposal-operations-heading"><div><span className="section-label">Resolved operations</span><h2>What will change</h2></div><span>{proposal.operationCount}</span></div>
      <ol>{proposal.operations.map((operation, index) => <li key={`${proposal.id}-${index}`}><span className="operation-number">{index + 1}</span><p><strong>{proposalOperationLabel(operation, products, placements)}</strong>{proposalOperationMeta(operation, products) && <small>{proposalOperationMeta(operation, products)}</small>}</p></li>)}</ol>
    </section>

    {revisionRequested && <div className="revision-note" role="status"><RotateCcw size={16}/><p><strong>Revision requested</strong><span>The proposal remains unchanged and uncommitted while you refine the brief with ChatGPT.</span></p></div>}

    <div className="proposal-actions">
      <button className="accept-proposal" type="button" onClick={onAccept}><CheckCircle2 size={17}/>Accept proposal</button>
      <div><button type="button" onClick={onRevise}><RotateCcw size={16}/>Revise</button><button type="button" onClick={onReject}><XCircle size={16}/>Reject</button></div>
      <p>Accept records one atomic change set at revision {proposal.revision}.</p>
    </div>
  </aside>;
}

function AppliedProposalReceipt({
  changeSet,
  headingRef,
}: {
  changeSet: ChangeSet;
  headingRef: Ref<HTMLHeadingElement>;
}) {
  const operationCount = changeSet.operations.length;
  return <section className="applied-proposal-receipt" aria-labelledby="applied-proposal-heading">
    <p className="sr-only" role="status">WebMCP proposal approved by {changeSet.actor}. {changeSet.reason}. Revision {changeSet.base_revision} to {changeSet.resulting_revision}. Change set {changeSet.id}.</p>
    <div className="applied-proposal-kicker"><CheckCircle2 size={16}/><span>Approved change set</span></div>
    <h2 id="applied-proposal-heading" ref={headingRef} tabIndex={-1}>WebMCP proposal approved</h2>
    <p>{changeSet.reason}</p>
    <dl>
      <div><dt>Actor</dt><dd>{changeSet.actor}</dd></div>
      <div><dt>Revision</dt><dd>{changeSet.base_revision} → {changeSet.resulting_revision}</dd></div>
      <div><dt>Change set</dt><dd>{changeSet.id}</dd></div>
      <div><dt>Operations</dt><dd>{operationCount}</dd></div>
    </dl>
  </section>;
}

export function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const sessionRef = useRef<PlanogramSession | undefined>(undefined);
  const interactionRef = useRef<{ kind: 'drag' | 'pan'; pointerId: number; x: number; y: number } | undefined>(undefined);
  const context = useUiStore(state => state.context);
  const selection = useUiStore(state => state.selection);
  const commandStatus = useUiStore(state => state.commandStatus);
  const error = useUiStore(state => state.error);
  const setContext = useUiStore(state => state.setContext);
  const selectStore = useUiStore(state => state.select);
  const setCommand = useUiStore(state => state.setCommand);
  const [unsupported, setUnsupported] = useState<string>();
  const [elevationInput, setElevationInput] = useState('');
  const [placementShelfInput, setPlacementShelfInput] = useState('');
  const [placementXInput, setPlacementXInput] = useState('');
  const [shelfDistribution, setShelfDistribution] = useState<ShelfDistribution>('space_evenly');
  const [zoomLabel, setZoomLabel] = useState(100);
  const [productQuery, setProductQuery] = useState('');
  const [brandFilter, setBrandFilter] = useState('All brands');
  const [stockingFilter, setStockingFilter] = useState<'all' | StockingMode>('all');
  const [selectedProductId, setSelectedProductId] = useState<string>();
  const [webmcpStatus, setWebmcpStatus] = useState<'loading' | 'ready' | 'unsupported' | 'error'>('loading');
  const [proposal, setProposal] = useState<SessionProposal>();
  const [appliedProposal, setAppliedProposal] = useState<{ changeSet: ChangeSet; source: ProposalApprovalSource }>();
  const [revisionRequested, setRevisionRequested] = useState(false);
  const appliedProposalHeadingRef = useRef<HTMLHeadingElement>(null);

  const shelves = useMemo(() => context ? allShelves(context) : [], [context]);
  const products = context?.products ?? [];
  const selectedShelf = selection?.kind === 'shelf' ? shelves.find(shelf => shelf.id === selection.id) : undefined;
  const selectedPlacement = selection?.kind === 'placement' ? context?.placements.find(placement => placement.id === selection.id) : undefined;
  const selectedPlacementProduct = products.find(product => product.id === selectedPlacement?.product_id);
  const selectedShelfPlacementCount = selectedShelf
    ? context?.placements.filter(placement => placement.shelf_id === selectedShelf.id).length ?? 0
    : 0;
  const targetShelfId = selectedShelf?.id ?? selectedPlacement?.shelf_id;
  const targetShelf = shelves.find(shelf => shelf.id === targetShelfId);
  const canAddToTargetShelf = targetShelf?.kind === 'adjustable';
  const brands = useMemo(() => Array.from(new Set(products.map(product => product.brand))), [products]);
  const filteredProducts = useMemo(() => searchProducts(products, {
    query: productQuery,
    brand: brandFilter === 'All brands' ? undefined : brandFilter,
    stocking_mode: stockingFilter === 'all' ? undefined : stockingFilter,
    limit: 50,
  }), [brandFilter, productQuery, products, stockingFilter]);
  const selectedProduct = filteredProducts.find(product => product.id === selectedProductId) ?? filteredProducts[0];

  const handleCommand = useCallback((result: CommandResult) => {
    const message = statusError(result);
    setCommand(message ? 'rejected' : 'applied', message);
  }, [setCommand]);

  const selectTarget = useCallback((target?: Selection) => {
    selectStore(target);
    setCommand('idle');
    sessionRef.current?.select(target);
  }, [selectStore, setCommand]);

  const issueMove = useCallback((shelf: Shelf, elevationSixteenths: number, source: MoveSource) => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current) return;
    setCommand('working');
    const result = session.moveShelf({
      versionId: current.version_id,
      shelfId: shelf.id,
      elevationSixteenths,
      expectedRevision: current.revision,
    }, source);
    return result;
  }, [setCommand]);

  const issuePlacementMove = useCallback((placement: Placement, targetShelfId: string, xSixteenths: number, source: MoveSource) => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current || !targetShelfId) return;
    setCommand('working');
    const result = session.movePlacement({
      versionId: current.version_id,
      placementId: placement.id,
      targetShelfId,
      xSixteenths,
      expectedRevision: current.revision,
    }, source);
    return result;
  }, [setCommand]);

  const issuePlacement = useCallback((productId: string, shelfId: string | undefined, source: PlacementSource) => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current || !shelfId) { setCommand('rejected', 'Select a shelf before adding a product.'); return; }
    const shelf = allShelves(current).find(candidate => candidate.id === shelfId);
    if (!shelf || shelf.kind === 'base_deck') { setCommand('rejected', 'Select an adjustable shelf before adding a product.'); return; }
    setCommand('working');
    return session.addPlacement({ versionId: current.version_id, productId, shelfId, expectedRevision: current.revision }, source);
  }, [setCommand]);

  const issueDistribution = useCallback((shelf: Shelf, distribution: ShelfDistribution) => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current) return;
    setCommand('working');
    return session.distributeShelf({
      versionId: current.version_id,
      shelfId: shelf.id,
      distribution,
      expectedRevision: current.revision,
    }, 'inspector');
  }, [setCommand]);

  const issueRemoval = useCallback((placementId: string, source: RemovalSource) => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current) return;
    setCommand('working');
    const result = session.removePlacement({ versionId: current.version_id, placementId, expectedRevision: current.revision }, source);
    const message = statusError(result);
    if (!message) {
      selectStore(undefined);
      session.select(undefined);
    }
    return result;
  }, [selectStore, setCommand]);

  const keyboardMove = useCallback((event: React.KeyboardEvent | KeyboardEvent, shelf = selectedShelf) => {
    if (!shelf || shelf.kind !== 'adjustable' || !['ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    const direction = event.key === 'ArrowUp' ? 1 : -1;
    issueMove(shelf, shelf.elevation + direction * 16, 'keyboard');
  }, [issueMove, selectedShelf]);

  const keyboardSelection = useCallback((event: React.KeyboardEvent, placementId = selectedPlacement?.id) => {
    if (placementId && ['Delete', 'Backspace'].includes(event.key)) {
      event.preventDefault();
      issueRemoval(placementId, 'keyboard');
      return;
    }
    if (placementId && ['ArrowLeft', 'ArrowRight'].includes(event.key)) {
      const placement = useUiStore.getState().context?.placements.find(item => item.id === placementId);
      if (!placement) return;
      event.preventDefault();
      const direction = event.key === 'ArrowLeft' ? -1 : 1;
      issuePlacementMove(placement, placement.shelf_id, placement.x + direction * 2, 'keyboard');
      return;
    }
    keyboardMove(event);
  }, [issuePlacementMove, issueRemoval, keyboardMove, selectedPlacement?.id]);

  useEffect(() => {
    let cancelled = false;
    let observer: ResizeObserver | undefined;
    let unregisterWebMcp: (() => Promise<void>) | undefined;
    (async () => {
      try {
        const wasm = await import('./wasm/pkg/planogram_wasm.js');
        await wasm.default();
        if (cancelled) return;
        const engine = new wasm.PlanogramEngine() as unknown as WasmEngine;
        const session = new PlanogramSession(engine, {
          onContext: setContext,
          onCommand: handleCommand,
          onProposal: next => {
            setProposal(next);
            if (next) setAppliedProposal(undefined);
          },
          onProposalApplied: (result, source) => setAppliedProposal({ changeSet: result.change_set, source }),
        });
        sessionRef.current = session;
        await engine.initialize_renderer(CANVAS_ID);
        if (cancelled) return;
        session.refresh();
        const registration = await registerPlanogramWebMcp(session, () => useUiStore.getState().selection);
        if (cancelled) {
          await registration.unregister?.();
          return;
        }
        setWebmcpStatus(registration.status);
        unregisterWebMcp = registration.unregister;
        const canvas = canvasRef.current!;
        observer = new ResizeObserver(entries => {
          const rect = entries[0].contentRect;
          engine.resize(Math.round(rect.width), Math.round(rect.height));
        });
        observer.observe(canvas);
      } catch (cause) {
        setUnsupported(cause instanceof Error ? cause.message : String(cause));
      }
    })();
    return () => {
      cancelled = true;
      observer?.disconnect();
      void unregisterWebMcp?.();
      sessionRef.current = undefined;
      setProposal(undefined);
      setAppliedProposal(undefined);
    };
  }, [handleCommand, setContext]);

  useEffect(() => {
    setElevationInput(selectedShelf ? formatImperial(selectedShelf.elevation) : '');
  }, [selectedShelf?.id, selectedShelf?.elevation]);

  useEffect(() => {
    setPlacementShelfInput(selectedPlacement?.shelf_id ?? '');
    setPlacementXInput(selectedPlacement ? formatImperial(selectedPlacement.x) : '');
  }, [selectedPlacement?.id, selectedPlacement?.shelf_id, selectedPlacement?.x]);

  useEffect(() => setRevisionRequested(false), [proposal?.id]);

  useEffect(() => {
    if (appliedProposal?.source === 'human') appliedProposalHeadingRef.current?.focus();
  }, [appliedProposal?.changeSet.id, appliedProposal?.source]);

  const pointerPosition = (event: React.PointerEvent | React.DragEvent) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: event.clientX - rect.left, y: event.clientY - rect.top };
  };

  const pointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const engine = sessionRef.current?.engine;
    if (!engine) return;
    const point = pointerPosition(event);
    const hit = engine.hit_test(point.x, point.y);
    event.currentTarget.setPointerCapture(event.pointerId);
    if (hit?.kind === 'placement') {
      selectTarget({ kind: 'placement', id: hit.id });
    } else if (hit?.kind === 'shelf') {
      selectTarget({ kind: 'shelf', id: hit.id });
      if (engine.begin_drag(hit.id, point.y)) interactionRef.current = { kind: 'drag', pointerId: event.pointerId, ...point };
    } else {
      selectTarget(undefined);
      interactionRef.current = { kind: 'pan', pointerId: event.pointerId, ...point };
    }
  };

  const pointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const interaction = interactionRef.current;
    const engine = sessionRef.current?.engine;
    if (!interaction || !engine || interaction.pointerId !== event.pointerId) return;
    const point = pointerPosition(event);
    if (interaction.kind === 'drag') engine.preview_drag(point.y);
    else { engine.pan_by(point.x - interaction.x, point.y - interaction.y); interaction.x = point.x; interaction.y = point.y; }
  };

  const pointerUp = (event: React.PointerEvent<HTMLCanvasElement>) => {
    const interaction = interactionRef.current;
    const engine = sessionRef.current?.engine;
    interactionRef.current = undefined;
    if (!interaction || !engine || interaction.pointerId !== event.pointerId) return;
    if (interaction.kind === 'drag') {
      const completed = engine.finish_drag();
      const shelf = shelves.find(item => item.id === completed?.[0]);
      if (completed && shelf && completed[1] !== shelf.elevation) issueMove(shelf, completed[1], 'pointer');
      else engine.cancel_drag();
    }
  };

  const cancelPointer = () => { interactionRef.current = undefined; sessionRef.current?.engine.cancel_drag(); };

  const changeZoom = (factor: number) => {
    sessionRef.current?.engine.zoom_by(factor);
    setZoomLabel(value => Math.max(35, Math.min(500, Math.round(value * factor))));
  };

  const fitFixture = () => { sessionRef.current?.engine.fit_fixture(); setZoomLabel(100); };

  const undo = () => {
    const session = sessionRef.current;
    if (!session || !context?.latest_change_set_id) return;
    setCommand('working');
    session.undoChangeSet({ versionId: context.version_id, changeSetId: context.latest_change_set_id, expectedRevision: context.revision }, 'human');
  };

  const acceptProposal = () => {
    const session = sessionRef.current;
    const current = useUiStore.getState().context;
    if (!session || !current || !proposal) return;
    setCommand('working');
    session.applyChanges({ versionId: current.version_id, proposalId: proposal.id, expectedRevision: current.revision }, 'human');
  };

  const rejectProposal = () => {
    if (!proposal || !sessionRef.current?.rejectProposal(proposal.id)) return;
    setCommand('idle');
  };

  const submitElevation = (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedShelf || selectedShelf.kind !== 'adjustable') return;
    const parsed = parseImperial(elevationInput);
    if (!parsed.ok) { setCommand('rejected', parsed.error); return; }
    issueMove(selectedShelf, parsed.sixteenths, 'inspector');
  };

  const submitPlacementMove = (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedPlacement || !placementShelfInput) return;
    const parsed = parseImperial(placementXInput);
    if (!parsed.ok) {
      setCommand('rejected', parsed.error);
      setPlacementShelfInput(selectedPlacement.shelf_id);
      setPlacementXInput(formatImperial(selectedPlacement.x));
      return;
    }
    const result = issuePlacementMove(selectedPlacement, placementShelfInput, parsed.sixteenths, 'inspector');
    if (result?.status !== 'applied') {
      setPlacementShelfInput(selectedPlacement.shelf_id);
      setPlacementXInput(formatImperial(selectedPlacement.x));
    }
  };

  const submitDistribution = (event: React.FormEvent) => {
    event.preventDefault();
    if (!selectedShelf || selectedShelf.kind !== 'adjustable' || selectedShelfPlacementCount === 0) return;
    issueDistribution(selectedShelf, shelfDistribution);
  };

  if (unsupported) return <main className="unsupported"><div><h1>WebGPU is required</h1><p>Planogrammy could not initialize its Rust WebGPU renderer.</p><code>{unsupported}</code></div></main>;

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand"><span className="brand-mark" aria-hidden="true">P</span><strong>Planogrammy</strong><span className="document-title">4' Standard Bay</span></div>
        <nav aria-label="Editor controls" className="toolbar">
          <button onClick={undo} disabled={!context?.latest_change_set_id} title="Undo last change"><Undo2 size={17}/>Undo</button>
          <button onClick={fitFixture}><Focus size={17}/>Fit fixture</button>
          <span className="toolbar-divider"/>
          <button className="icon-button" onClick={() => changeZoom(0.9)} aria-label="Zoom out"><Minus size={17}/></button>
          <output className="zoom-value" aria-label="Zoom level">{zoomLabel}%</output>
          <button className="icon-button" onClick={() => changeZoom(1.1)} aria-label="Zoom in"><Plus size={17}/></button>
        </nav>
      </header>

      <section className="workspace">
        <aside className="catalog" aria-label="Product catalog">
          <div className="catalog-header"><div><span className="section-label">Assortment</span><h1>Product library</h1></div><span>{filteredProducts.length} SKUs</span></div>
          <label className="catalog-search"><Search size={15}/><span className="sr-only">Search products</span><input value={productQuery} onChange={event => setProductQuery(event.target.value)} placeholder="Search products"/></label>
          <div className="catalog-filters">
            <label><span className="sr-only">Filter by brand</span><select value={brandFilter} onChange={event => setBrandFilter(event.target.value)}><option>All brands</option>{brands.map(brand => <option key={brand}>{brand}</option>)}</select></label>
            <label><span className="sr-only">Filter by stocking mode</span><select value={stockingFilter} onChange={event => setStockingFilter(event.target.value as 'all' | StockingMode)}><option value="all">All stocking</option><option value="loose">Loose</option><option value="tray">Tray stocked</option></select></label>
          </div>
          <p className="catalog-data-note">Trailing 13-week illustrative performance · synthetic, not retailer actuals</p>
          <div className="product-list">
            {filteredProducts.map(product => <button
              key={product.id}
              type="button"
              className={`product-row ${selectedProduct?.id === product.id ? 'selected' : ''}`}
              onClick={() => setSelectedProductId(product.id)}
              onDoubleClick={() => issuePlacement(product.id, targetShelfId, 'catalog_double_click')}
              draggable
              onDragStart={event => { event.dataTransfer.setData('application/x-planogram-product', product.id); event.dataTransfer.effectAllowed = 'copy'; setSelectedProductId(product.id); }}
              aria-pressed={selectedProduct?.id === product.id}
              title={productTooltip(product)}
            >
              <ProductSwatch product={product}/>
              <span className="product-copy">
                <strong>{product.brand} {product.description}</strong>
                <small>{product.size_oz} · {formatImperial(product.dimensions.width)} W × {formatImperial(product.dimensions.height)} H × {formatImperial(product.dimensions.depth)} D</small>
                <span className="catalog-metrics" aria-label={`${formatSalesPerStoreWeek(product)} sales per store per week, ${formatUnitsPerStoreWeek(product)} units per store per week, ${formatGrossMargin(product)} gross margin`}><span>{formatSalesPerStoreWeek(product)} SSW</span><span>{formatUnitsPerStoreWeek(product)} USW</span><span>{formatGrossMargin(product)} GM</span></span>
                <small className="catalog-logistics">{formatNetWeight(product)} net · Casepack {product.casepack_quantity}{product.tray ? ` · Tray ${product.tray.facings_x}×${product.tray.units_deep}` : ' · Loose'}</small>
                {product.tray && <small className="catalog-tray">Loaded {formatImperial(product.tray.outer_width_sixteenths)} W × {formatImperial(product.tray.outer_height_sixteenths)} H × {formatImperial(product.tray.outer_depth_sixteenths)} D · lip {formatImperial(product.tray.front_lip_height_sixteenths)}</small>}
                <small className="upc">UPC {product.upc}</small>
                <span className="sr-only">{productAccessibilitySummary(product)}</span>
              </span>
            </button>)}
            {!filteredProducts.length && <p className="catalog-empty">No products match this search.</p>}
          </div>
          <div className="catalog-action"><button onClick={() => selectedProduct && issuePlacement(selectedProduct.id, targetShelfId, 'catalog_button')} disabled={!selectedProduct || !canAddToTargetShelf}><PackagePlus size={16}/><span>{selectedProduct?.tray ? 'Add tray to selected shelf' : 'Add to selected shelf'}<small>{selectedProduct ? selectedProduct.tray ? `${SHELF_READY_TRAY_LABEL} · ${selectedProduct.tray.facings_x} facings × ${selectedProduct.tray.units_deep} deep` : `${selectedProduct.brand} ${selectedProduct.size_oz} · loose` : 'Choose a product'}</small></span></button><p>Double-click or drag a product onto a shelf. Tray presets resolve in Rust.</p></div>
        </aside>
        <div className="canvas-region">
          <div className="canvas-heading"><div><span>Front elevation</span><strong>Section 01</strong></div><div className="dimensions">4' W × 7' H × 22" D</div></div>
          <canvas
            ref={canvasRef}
            id={CANVAS_ID}
            tabIndex={0}
            aria-label="Planogram fixture canvas. Use the companion panel for an accessible fixture outline."
            onKeyDown={keyboardSelection}
            onPointerDown={pointerDown}
            onPointerMove={pointerMove}
            onPointerUp={pointerUp}
            onPointerCancel={cancelPointer}
            onLostPointerCapture={cancelPointer}
            onWheel={event => { event.preventDefault(); changeZoom(event.deltaY < 0 ? 1.08 : 0.92); }}
            onDragOver={event => { if (event.dataTransfer.types.includes('application/x-planogram-product')) { event.preventDefault(); event.dataTransfer.dropEffect = 'copy'; } }}
            onDrop={event => { event.preventDefault(); const productId = event.dataTransfer.getData('application/x-planogram-product'); const point = pointerPosition(event); const hit = sessionRef.current?.engine.hit_test(point.x, point.y); const shelfId = hit?.kind === 'shelf' ? hit.id : hit?.shelf_id; if (productId) { if (shelfId) selectTarget({ kind: 'shelf', id: shelfId }); issuePlacement(productId, shelfId, 'catalog_drag'); } }}
          />
          {!proposal && <div className="canvas-help">Select products or shelves · Arrows move selected products 1/8" · Drag shelves at 1" · Scroll to zoom</div>}
          {proposal && <div className="proposal-legend" aria-label="Proposal preview legend"><span><Package size={13}/>Current placement</span><span><PackagePlus size={13}/>Proposed position</span></div>}
        </div>

        {proposal ? <ProposalReview proposal={proposal} products={products} placements={context?.placements ?? []} revisionRequested={revisionRequested} onAccept={acceptProposal} onRevise={() => setRevisionRequested(true)} onReject={rejectProposal}/> : <aside className="inspector" aria-label="Selection inspector">
          {appliedProposal && <AppliedProposalReceipt changeSet={appliedProposal.changeSet} headingRef={appliedProposalHeadingRef}/>}
          <section className="inspector-section selection-panel">
            <span className="section-label">Selection</span>
            {selectedShelf ? <>
              <h1>{selectedShelf.kind === 'base_deck' ? 'Base deck' : selectedShelf.id.replace('_', ' ').replace(/\b\w/g, char => char.toUpperCase())}</h1>
              <dl><div><dt>ID</dt><dd>{selectedShelf.id}</dd></div><div><dt>Kind</dt><dd>{selectedShelf.kind === 'base_deck' ? 'Base deck' : 'Adjustable'}</dd></div><div><dt>Depth</dt><dd>{formatImperial(selectedShelf.depth)}</dd></div><div><dt>Elevation</dt><dd>{formatImperial(selectedShelf.elevation)}</dd></div></dl>
              <form onSubmit={submitElevation} className="elevation-form">
                <label htmlFor="elevation">Elevation</label>
                <div className="field-row"><input id="elevation" value={elevationInput} onChange={event => setElevationInput(event.target.value)} disabled={selectedShelf.kind === 'base_deck'} aria-describedby="elevation-help elevation-error"/><button disabled={selectedShelf.kind === 'base_deck'}>Apply</button></div>
                <small id="elevation-help">Arrow keys move shelves in 1" increments</small>
                {error && <p id="elevation-error" className="error" role="alert">{error}</p>}
              </form>
              <form onSubmit={submitDistribution} className="distribution-form">
                <label htmlFor="shelf-distribution">Product distribution</label>
                <div className="field-row">
                  <select id="shelf-distribution" value={shelfDistribution} onChange={event => setShelfDistribution(event.target.value as ShelfDistribution)} disabled={selectedShelf.kind === 'base_deck' || selectedShelfPlacementCount === 0} aria-describedby="distribution-help distribution-error">
                    <option value="packed_left">Pack left</option>
                    <option value="centered">Center group</option>
                    <option value="space_between">Space between</option>
                    <option value="space_evenly">Space evenly</option>
                  </select>
                  <button disabled={selectedShelf.kind === 'base_deck' || selectedShelfPlacementCount === 0}>Apply</button>
                </div>
                <small id="distribution-help">Rust preserves left-to-right order and at least 1/8&quot; between products. {selectedShelfPlacementCount} {selectedShelfPlacementCount === 1 ? 'product' : 'products'} on this shelf.</small>
                {error && <p id="distribution-error" className="error" role="alert">{error}</p>}
              </form>
            </> : selectedPlacement && selectedPlacementProduct ? <>
              <h1>{selectedPlacementProduct.brand} {selectedPlacementProduct.description}</h1>
              <span className={`stocking-badge ${selectedPlacement.stocking_mode}`}>{selectedPlacement.stocking_mode === 'tray' ? SHELF_READY_TRAY_LABEL : 'Loose stocked'}</span>
              <h2 className="inspector-subheading">Placement</h2>
              <dl>
                <div><dt>ID</dt><dd>{selectedPlacement.id}</dd></div>
                <div><dt>Shelf</dt><dd>{selectedPlacement.shelf_id}</dd></div>
                <div><dt>Position</dt><dd>{formatImperial(selectedPlacement.x)}</dd></div>
                <div><dt>Stocking</dt><dd>{selectedPlacement.stocking_mode === 'tray' ? 'Tray' : 'Loose'}</dd></div>
                <div><dt>Resolved facings</dt><dd>{selectedPlacement.facings_x} × {selectedPlacement.facings_y} × {selectedPlacement.facings_z}</dd></div>
                <div><dt>Stocked units</dt><dd>{selectedPlacement.stocked_unit_count}</dd></div>
                <div><dt>Loaded footprint</dt><dd>{formatImperial(selectedPlacement.geometry.display_width)} W × {formatImperial(selectedPlacement.geometry.display_height)} H × {formatImperial(selectedPlacement.geometry.required_depth)} D</dd></div>
              </dl>
              <h2 className="inspector-subheading">Item logistics</h2>
              <dl>
                <div><dt>Net size</dt><dd>{selectedPlacementProduct.size_oz}</dd></div>
                <div><dt>Net weight</dt><dd>{formatNetWeight(selectedPlacementProduct)}</dd></div>
                <div><dt>Unit dimensions</dt><dd>{formatImperial(selectedPlacementProduct.dimensions.width)} W × {formatImperial(selectedPlacementProduct.dimensions.height)} H × {formatImperial(selectedPlacementProduct.dimensions.depth)} D</dd></div>
                <div><dt>Casepack</dt><dd>{selectedPlacementProduct.casepack_quantity}</dd></div>
              </dl>
              {selectedPlacementProduct.tray && <>
                <h2 className="inspector-subheading">Tray configuration</h2>
                <dl>
                  <div><dt>Preset</dt><dd>{selectedPlacementProduct.tray.facings_x} facings × {selectedPlacementProduct.tray.units_deep} deep</dd></div>
                  <div><dt>Loaded envelope</dt><dd>{formatImperial(selectedPlacementProduct.tray.outer_width_sixteenths)} W × {formatImperial(selectedPlacementProduct.tray.outer_height_sixteenths)} H × {formatImperial(selectedPlacementProduct.tray.outer_depth_sixteenths)} D</dd></div>
                  <div><dt>Front lip</dt><dd>{formatImperial(selectedPlacementProduct.tray.front_lip_height_sixteenths)}</dd></div>
                </dl>
              </>}
              <h2 className="inspector-subheading">Performance</h2>
              <dl>
                <div><dt>Sales / store / week</dt><dd>{formatSalesPerStoreWeek(selectedPlacementProduct)}</dd></div>
                <div><dt>Units / store / week</dt><dd>{formatUnitsPerStoreWeek(selectedPlacementProduct)}</dd></div>
                <div><dt>Gross margin</dt><dd>{formatGrossMargin(selectedPlacementProduct)}</dd></div>
                <div><dt>Period</dt><dd>{selectedPlacementProduct.performance.period}</dd></div>
              </dl>
              <p className="performance-source"><strong>Illustrative data</strong><span>{selectedPlacementProduct.performance.source}</span></p>
              <div className="placement-move-controls" aria-label="Placement movement">
                <button type="button" onClick={() => issuePlacementMove(selectedPlacement, selectedPlacement.shelf_id, selectedPlacement.x - 2, 'inspector')} aria-label="Move left 1/8 inch">← <span>Left <small>1/8&quot;</small></span></button>
                <button type="button" onClick={() => issuePlacementMove(selectedPlacement, selectedPlacement.shelf_id, selectedPlacement.x + 2, 'inspector')} aria-label="Move right 1/8 inch"><span>Right <small>1/8&quot;</small></span> →</button>
              </div>
              <form onSubmit={submitPlacementMove} className="placement-form">
                <label htmlFor="placement-shelf">Shelf</label>
                <select id="placement-shelf" value={placementShelfInput} onChange={event => setPlacementShelfInput(event.target.value)} aria-describedby="placement-help placement-error">
                  {shelves.map(shelf => <option key={shelf.id} value={shelf.id} disabled={shelf.kind === 'base_deck'}>{shelf.kind === 'base_deck' ? 'Base deck (fixed)' : shelf.id.replace('shelf_', 'Shelf ')}</option>)}
                </select>
                <label htmlFor="placement-x">Position</label>
                <div className="field-row"><input id="placement-x" value={placementXInput} onChange={event => setPlacementXInput(event.target.value)} aria-describedby="placement-help placement-error"/><button>Apply</button></div>
                <small id="placement-help">Arrow keys move left or right in exact 1/8&quot; increments. Apply changes the shelf and position together.</small>
                {error && <p id="placement-error" className="error" role="alert">{error}</p>}
              </form>
              <button className="destructive-action" onClick={() => issueRemoval(selectedPlacement.id, 'inspector')}><Trash2 size={16}/>Remove product</button>
              <small className="keyboard-hint">Delete or Backspace also removes this placement.</small>
            </> : <div className="empty-selection"><RotateCcw size={21}/><p>Select a shelf or product on the canvas or in the fixture outline.</p></div>}
          </section>

          <section className="inspector-section companion" aria-labelledby="companion-heading">
            <div className="companion-heading"><div><span className="section-label">Accessible companion</span><h2 id="companion-heading">Fixture outline</h2></div><span>{shelves.length} levels</span></div>
            <p className="sr-only">Fixture width {context && formatImperial(context.fixture.width)} and height {context && formatImperial(context.fixture.height)}. Current revision {context?.revision ?? 0}. Placement entries identify loose or tray stocking, resolved facings, stocked units, and loaded footprint. Keyboard commands: arrow keys move a selected adjustable shelf one inch or move a selected placement left and right in 1/8-inch increments. Placement shelf and position can be changed together in the inspector. Selected shelves can pack, center, space between, or space products evenly while keeping a 1/8-inch minimum gap. Delete or Backspace removes a selected product placement. The base deck is fixed.</p>
            <ol className="shelf-list">
              {shelves.map(shelf => { const shelfPlacements = context?.placements.filter(placement => placement.shelf_id === shelf.id) ?? []; return <li key={shelf.id}><button className={selection?.kind === 'shelf' && shelf.id === selection.id ? 'selected' : ''} onClick={() => selectTarget({ kind: 'shelf', id: shelf.id })} onKeyDown={event => keyboardMove(event, shelf)} aria-current={selection?.kind === 'shelf' && shelf.id === selection.id ? 'true' : undefined}><span><strong>{shelf.kind === 'base_deck' ? 'Base deck' : shelf.id.replace('shelf_', 'Shelf ')}</strong><small>{shelf.kind === 'base_deck' ? 'Fixed · 22" deep' : 'Adjustable · 16" deep'} · {shelfPlacements.length} {shelfPlacements.length === 1 ? 'placement' : 'placements'}</small></span><output>{formatImperial(shelf.elevation)}</output></button>{shelfPlacements.length > 0 && <ul className="placement-list">{shelfPlacements.map(placement => { const product = products.find(item => item.id === placement.product_id); const label = `${product?.brand ?? 'Product'} ${product?.description ?? placement.id}`; return <li key={placement.id}><div className="companion-placement"><button className={selection?.kind === 'placement' && placement.id === selection.id ? 'selected' : ''} onClick={() => selectTarget({ kind: 'placement', id: placement.id })} onKeyDown={event => keyboardSelection(event, placement.id)} aria-current={selection?.kind === 'placement' && placement.id === selection.id ? 'true' : undefined}><span><strong>{label}</strong><small>{product?.size_oz} · at {formatImperial(placement.x)} · {placementStockingLabel(placement)} · footprint {formatImperial(placement.geometry.display_width)} W × {formatImperial(placement.geometry.display_height)} H × {formatImperial(placement.geometry.required_depth)} D</small>{product && <span className="sr-only">{productAccessibilitySummary(product)}</span>}</span></button><div className="companion-placement-actions" aria-label={`${label} movement controls`}><button type="button" onClick={() => issuePlacementMove(placement, placement.shelf_id, placement.x - 2, 'inspector')} aria-label={`Move ${label} left 1/8 inch`}>←</button><button type="button" onClick={() => issuePlacementMove(placement, placement.shelf_id, placement.x + 2, 'inspector')} aria-label={`Move ${label} right 1/8 inch`}>→</button></div></div></li>; })}</ul>}</li>; })}
            </ol>
          </section>
        </aside>}
      </section>

      <footer className="statusbar"><span>Revision {context?.revision ?? 0} · All changes local</span><span className={`site-tools-status ${webmcpStatus}`} aria-live="polite">{webmcpStatus === 'ready' ? 'Site tools ready' : webmcpStatus === 'unsupported' ? 'Site tools unavailable' : webmcpStatus === 'error' ? 'Site tools error' : 'Site tools loading'}</span>{proposal && <span className="proposal-status" aria-live="polite" title={proposal.reason}>Proposal ready · {proposal.operationCount} changes</span>}<span className={`command-status ${commandStatus}`}>{commandStatus === 'rejected' ? 'Change rejected' : commandStatus === 'applied' ? 'Change recorded' : 'Ready'}</span></footer>
    </main>
  );
}
