import { useEffect, useRef, useState, type Ref } from 'react';
import { ChartNoAxesCombined, ScanEye } from 'lucide-react';
import { formatImperial } from './imperial';
import type { EngineContext, Product, SalesAllocationBasis, SalesAllocationReport, SalesAllocationRequest, SalesAllocationTarget, Shelf } from './types';

export interface SalesAllocationOptions {
  basis: SalesAllocationBasis;
  target: SalesAllocationTarget;
  scope: 'shelf' | 'bay';
  minFacings: string;
  maxFacings: string;
}

export const DEFAULT_SALES_ALLOCATION_OPTIONS: SalesAllocationOptions = {
  basis: 'revenue',
  target: 'space',
  scope: 'shelf',
  minFacings: '1',
  maxFacings: '100',
};

function shelfName(id: string) {
  return id.replace(/^bay_(\d+)_/, 'Bay $1 · ').replace('shelf_', 'Shelf ').replace('base_deck', 'Base deck');
}

/** Reveal feedback inside the scrollable inspector without moving the app viewport. */
export function focusWithinInspector(element: HTMLElement | null, resetScroll = false) {
  if (!element) return;
  element.focus({ preventScroll: true });
  const panel = element.closest<HTMLElement>('.inspector, .proposal-review');
  if (!panel) return;
  if (resetScroll) { panel.scrollTop = 0; return; }
  const bounds = panel.getBoundingClientRect();
  const target = element.getBoundingClientRect();
  if (target.top < bounds.top + 12) panel.scrollTop += target.top - bounds.top - 12;
  else if (target.bottom > bounds.bottom - 12) panel.scrollTop += target.bottom - bounds.bottom + 12;
}

export function SalesAllocation({
  context,
  shelf,
  options,
  onOptionsChange,
  onPreview,
  onInvalid,
  previewButtonRef,
}: {
  context: EngineContext;
  shelf: Shelf;
  options: SalesAllocationOptions;
  onOptionsChange: (options: SalesAllocationOptions) => void;
  onPreview: (request: SalesAllocationRequest) => string | undefined;
  onInvalid: (message: string) => void;
  previewButtonRef?: Ref<HTMLButtonElement>;
}) {
  const [error, setError] = useState<string>();
  const [invalidLimits, setInvalidLimits] = useState(false);
  const errorRef = useRef<HTMLParagraphElement>(null);
  const readOnly = context.version_status !== 'draft' && context.version_status !== 'proposed';
  const fixedShelf = shelf.kind !== 'adjustable';
  const scopedShelves = options.scope === 'shelf'
    ? [shelf]
    : context.fixture.sections.find(section => section.id === shelf.section_id)?.shelves.filter(item => item.kind === 'adjustable') ?? [];
  const shelfIds = new Set(scopedShelves.map(item => item.id));
  const placements = context.placements.filter(placement => shelfIds.has(placement.shelf_id));
  const looseCount = placements.filter(placement => placement.stocking_mode === 'loose').length;
  const productCount = new Set(placements.map(placement => placement.product_id)).size;
  const trayCount = placements.length - looseCount;
  const occupiedShelfCount = new Set(placements.map(placement => placement.shelf_id)).size;
  const sectionIndex = context.fixture.sections.findIndex(section => section.id === shelf.section_id);
  const scopeLabel = options.scope === 'shelf' ? shelfName(shelf.id) : `Bay ${String(sectionIndex + 1).padStart(2, '0')}`;
  const unavailable = readOnly
    ? 'This baseline is locked. Switch to an editable alternative to preview an allocation.'
    : fixedShelf
      ? 'Select an adjustable shelf or one of its products to allocate sales contribution.'
      : placements.length === 0
        ? options.scope === 'shelf'
          ? 'This shelf is empty. Add products, or choose Selected bay to use its occupied shelves.'
          : 'This bay is empty. Add products to an adjustable shelf first.'
        : looseCount === 0
          ? 'This scope contains only loaded trays. Their preset facings are fixed; choose a scope with loose products.'
          : undefined;

  useEffect(() => { if (error) focusWithinInspector(errorRef.current); }, [error]);
  useEffect(() => { setError(undefined); setInvalidLimits(false); }, [context.version_id, context.revision, shelf.id]);

  const change = (next: Partial<SalesAllocationOptions>) => {
    setError(undefined);
    setInvalidLimits(false);
    onOptionsChange({ ...options, ...next });
  };
  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (unavailable) return;
    const minimum = options.minFacings.trim();
    const maximum = options.maxFacings.trim();
    let message: string | undefined;
    if (![minimum, maximum].every(value => /^\d+$/.test(value) && Number(value) >= 1 && Number(value) <= 100)) {
      message = 'Minimum and maximum facings must be whole numbers from 1 to 100.';
    } else if (Number(minimum) > Number(maximum)) {
      message = 'Minimum facings cannot exceed maximum facings.';
    }
    if (message) {
      setInvalidLimits(true);
      setError(message);
      onInvalid(message);
      return;
    }
    setInvalidLimits(false);
    setError(onPreview({
      scope: options.scope === 'shelf' ? { kind: 'shelf', shelf_id: shelf.id } : { kind: 'bay', section_id: shelf.section_id },
      basis: options.basis,
      target: options.target,
      min_facings: Number(minimum),
      max_facings: Number(maximum),
    }));
  };

  return <section className="sales-allocation" aria-labelledby="sales-allocation-heading">
    <div className="sales-allocation-heading"><ChartNoAxesCombined size={18} aria-hidden="true"/><h2 id="sales-allocation-heading">Sales allocation</h2><span>Synthetic</span></div>
    <p className="sales-allocation-intro">Balance shelf space or facing counts using each product’s share of sales.</p>
    <form onSubmit={submit} noValidate>
      <fieldset disabled={readOnly || fixedShelf}>
        <legend className="sr-only">Sales allocation options</legend>
        <div className="sales-allocation-fields">
          <label htmlFor="sales-allocation-basis">Sales contribution<select id="sales-allocation-basis" aria-label="Sales contribution" value={options.basis} onChange={event => change({ basis: event.target.value as SalesAllocationBasis })} aria-describedby="sales-allocation-basis-help"><option value="revenue">Revenue</option><option value="units">Units</option></select></label>
          <label htmlFor="sales-allocation-target">Allocate<select id="sales-allocation-target" aria-label="Allocate" value={options.target} onChange={event => change({ target: event.target.value as SalesAllocationTarget })} aria-describedby="sales-allocation-target-help"><option value="space">Shelf space</option><option value="facings">Horizontal facings</option></select></label>
        </div>
        <p className="sales-allocation-help"><span id="sales-allocation-basis-help">{options.basis === 'revenue' ? 'Revenue uses sales dollars.' : 'Units uses quantities sold.'}</span> <span id="sales-allocation-target-help">{options.target === 'space' ? 'Space compares product widths, excluding gaps.' : 'Facings compares front-facing counts, regardless of product width.'}</span></p>
        <label className="sales-allocation-scope" htmlFor="sales-allocation-scope">Allocation scope<select id="sales-allocation-scope" aria-label="Allocation scope" value={options.scope} onChange={event => change({ scope: event.target.value as SalesAllocationOptions['scope'] })} aria-describedby="sales-allocation-scope-help"><option value="shelf">Selected shelf</option><option value="bay">Selected bay</option></select></label>
        <p id="sales-allocation-scope-help" className="sales-allocation-help">{scopeLabel} · {productCount} {productCount === 1 ? 'SKU' : 'SKUs'}{options.scope === 'bay' ? ` across ${occupiedShelfCount} occupied ${occupiedShelfCount === 1 ? 'shelf' : 'shelves'}` : ''}{trayCount > 0 ? ` · ${trayCount} ${trayCount === 1 ? 'tray' : 'trays'} with fixed facings` : ''}</p>
        <div className="sales-allocation-fields sales-allocation-limits">
          <label htmlFor="sales-allocation-min">Minimum facings<input id="sales-allocation-min" type="text" inputMode="numeric" value={options.minFacings} onChange={event => change({ minFacings: event.target.value })} aria-invalid={invalidLimits} aria-describedby={error ? 'sales-allocation-limits-help sales-allocation-error' : 'sales-allocation-limits-help'}/></label>
          <label htmlFor="sales-allocation-max">Maximum facings<input id="sales-allocation-max" type="text" inputMode="numeric" value={options.maxFacings} onChange={event => change({ maxFacings: event.target.value })} aria-invalid={invalidLimits} aria-describedby={error ? 'sales-allocation-limits-help sales-allocation-error' : 'sales-allocation-limits-help'}/></label>
        </div>
        <p id="sales-allocation-limits-help" className="sales-allocation-help">1–100 per loose placement. A SKU placed twice keeps both placements.</p>
        <button ref={previewButtonRef} className="sales-allocation-preview" type="submit" disabled={!!unavailable} aria-describedby="sales-allocation-preserves sales-allocation-disclaimer"><ScanEye size={16} aria-hidden="true"/>Preview sales allocation</button>
      </fieldset>
      {unavailable && <p className="sales-allocation-unavailable" role="status">{unavailable}</p>}
      {error && <p ref={errorRef} id="sales-allocation-error" className="error" role="alert" tabIndex={-1}>{error}</p>}
    </form>
    <p id="sales-allocation-preserves" className="sales-allocation-preserves">Keeps every placed SKU, shelf assignment and product order. Changes horizontal facings and spacing; stacking, depth and tray presets stay fixed.</p>
    <p id="sales-allocation-disclaimer" className="sales-allocation-disclaimer">Synthetic inputs only. Review the fit before accepting. This is an allocation plan, not a sales forecast.</p>
  </section>;
}

const shareFormatter = new Intl.NumberFormat('en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
function share(basisPoints: number) { return `${shareFormatter.format(basisPoints / 100)}%`; }

export function SalesAllocationReview({ report, products }: { report: SalesAllocationReport; products: Product[] }) {
  const [query, setQuery] = useState('');
  const basisLabel = report.basis === 'revenue' ? 'Revenue' : 'Units';
  const targetLabel = report.target === 'space' ? 'space' : 'facings';
  const scopeLabel = report.scope.kind === 'shelf' ? shelfName(report.scope.shelf_id) : report.scope.section_id.replace(/^(bay|section)_(\d+)$/, 'Bay $2');
  const productMap = new Map(products.map(product => [product.id, product]));
  const matchingRows = report.rows.filter(row => {
    const product = productMap.get(row.product_id);
    return `${product?.brand ?? ''} ${product?.description ?? ''} ${product?.size_oz ?? ''} ${row.product_id}`.toLowerCase().includes(query.trim().toLowerCase());
  });

  return <section className="sales-allocation-report" aria-labelledby="sales-allocation-report-heading">
    <div className="sales-report-heading"><span className="section-label">Synthetic sales inputs</span><h2 id="sales-allocation-report-heading">Sales allocation preview</h2></div>
    <p className="sales-report-summary">{scopeLabel} · {basisLabel} contribution → {targetLabel}<br/>{report.product_count} SKUs · {report.shelf_count} {report.shelf_count === 1 ? 'shelf' : 'shelves'}</p>
    <p className="sales-report-source"><strong>{report.period}</strong><span>{report.source}</span></p>
    <p className="sales-report-explanation">Contribution counts each SKU once across the scope. Allocation shares include all represented products{report.fixed_tray_count > 0 ? ', including trays with fixed facings' : ''}{report.target === 'space' ? ', and exclude gaps' : ''}.</p>
    {report.rows.length > 8 && <label className="sales-report-search">Filter allocation products<input type="search" value={query} onChange={event => setQuery(event.target.value)} placeholder="Find a product in this preview"/></label>}
    <div className="sales-report-table-region" role="region" aria-label="Sales allocation by product" tabIndex={0}>
      <table aria-label="Sales allocation by product">
        <caption className="sr-only">{basisLabel} contribution compared with each product’s share of {targetLabel} before and after allocation. Facings are horizontal totals across the scope.</caption>
        <thead><tr><th scope="col">Product</th><th scope="col">{basisLabel}<small>share</small></th><th scope="col">Before<small>{targetLabel}</small></th><th scope="col">Preview<small>{targetLabel}</small></th></tr></thead>
        <tbody>{matchingRows.map(row => {
          const product = productMap.get(row.product_id);
          return <tr key={row.product_id} data-product-id={row.product_id}>
            <th scope="row"><strong>{product ? product.description.startsWith(`${product.brand} `) ? product.description : `${product.brand} ${product.description}` : row.product_id}</strong>{product && <small>{product.size_oz}</small>}</th>
            <td>{share(row.contribution_basis_points)}</td>
            <td>{share(row.before_share_basis_points)}<small>{row.before_facings} {row.before_facings === 1 ? 'facing' : 'facings'}</small>{report.target === 'space' && <small>{formatImperial(row.before_space_sixteenths)}</small>}</td>
            <td className="sales-report-after">{share(row.after_share_basis_points)}<small>{row.after_facings} {row.after_facings === 1 ? 'facing' : 'facings'}</small>{report.target === 'space' && <small>{formatImperial(row.after_space_sixteenths)}</small>}</td>
          </tr>;
        })}</tbody>
      </table>
      {matchingRows.length === 0 && <p className="sales-report-empty">No products match this filter.</p>}
    </div>
    {query && <p className="sales-allocation-help" role="status">Showing {matchingRows.length} of {report.rows.length} products.</p>}
    {report.zero_weight_sku_count > 0 && <p className="sales-report-limit">{report.zero_weight_sku_count} {report.zero_weight_sku_count === 1 ? 'SKU has' : 'SKUs have'} zero {report.basis === 'revenue' ? 'revenue' : 'unit'} contribution. Existing representation is preserved.</p>}
    {report.warnings.length > 0 && <ul className="sales-report-warnings" aria-label="Allocation constraints">{report.warnings.map((warning, index) => <li key={`${index}-${warning}`}>{warning}</li>)}</ul>}
    <p className="sales-report-limit">Whole facings, fit limits and fixed shelf assignments can prevent an exact match. This deterministic allocation is an approximation; synthetic demand stays unchanged.</p>
  </section>;
}
