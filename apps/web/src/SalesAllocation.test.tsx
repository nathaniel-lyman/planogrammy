import { useState } from 'react';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_SALES_ALLOCATION_OPTIONS, SalesAllocation, SalesAllocationReview, type SalesAllocationOptions } from './SalesAllocation';
import { adjustableShelf, baseDeck, loosePlacement, looseProduct, makeContext, readySalesAllocationPreview, trayPlacement, trayProduct } from './testFixtures';
import type { EngineContext, SalesAllocationRequest, Shelf } from './types';

afterEach(cleanup);

const shelf = adjustableShelf('shelf_01', 192);
const secondShelf = adjustableShelf('shelf_02', 384);
const context = makeContext({ shelves: [baseDeck, shelf, secondShelf], products: [looseProduct, trayProduct], placements: [loosePlacement(), trayPlacement()] });

function renderForm(overrides: { context?: EngineContext; shelf?: Shelf; options?: SalesAllocationOptions; onPreview?: (request: SalesAllocationRequest) => string | undefined } = {}) {
  const onPreview = vi.fn(overrides.onPreview ?? (() => undefined));
  const onInvalid = vi.fn();
  function Harness() {
    const [options, setOptions] = useState(overrides.options ?? DEFAULT_SALES_ALLOCATION_OPTIONS);
    return <SalesAllocation context={overrides.context ?? context} shelf={overrides.shelf ?? shelf} options={options} onOptionsChange={setOptions} onPreview={onPreview} onInvalid={onInvalid}/>;
  }
  render(<Harness/>);
  return { onPreview, onInvalid };
}

describe('Sales allocation inspector', () => {
  it('sends only semantic scope, contribution and allocation limits', () => {
    const { onPreview } = renderForm();
    fireEvent.change(screen.getByLabelText('Sales contribution', { exact: true }), { target: { value: 'units' } });
    fireEvent.change(screen.getByLabelText('Allocate', { exact: true }), { target: { value: 'facings' } });
    fireEvent.change(screen.getByLabelText('Allocation scope', { exact: true }), { target: { value: 'bay' } });
    fireEvent.change(screen.getByLabelText('Minimum facings'), { target: { value: ' 2 ' } });
    fireEvent.change(screen.getByLabelText('Maximum facings'), { target: { value: '8' } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(onPreview).toHaveBeenCalledWith({ scope: { kind: 'bay', section_id: 'section_01' }, basis: 'units', target: 'facings', min_facings: 2, max_facings: 8 });
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it.each([
    ['', '5'], ['1.5', '5'], ['0', '5'], ['1', '101'], ['-1', '8'], ['1', 'NaN'], ['9', '3'],
  ])('rejects limits %s–%s without previewing', (minFacings, maxFacings) => {
    const { onPreview, onInvalid } = renderForm({ options: { ...DEFAULT_SALES_ALLOCATION_OPTIONS, minFacings, maxFacings } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(onPreview).not.toHaveBeenCalled();
    expect(onInvalid).toHaveBeenCalledOnce();
    expect(document.activeElement).toBe(screen.getByRole('alert'));
    expect(screen.getByLabelText('Minimum facings').getAttribute('aria-invalid')).toBe('true');
  });

  it('shows and focuses an authoritative infeasibility error, then allows a fresh preview', () => {
    const { onPreview } = renderForm({ onPreview: () => 'Minimum facings do not fit Shelf 01.' });
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(screen.getByRole('alert').textContent).toBe('Minimum facings do not fit Shelf 01.');
    expect(document.activeElement).toBe(screen.getByRole('alert'));
    fireEvent.change(screen.getByLabelText('Maximum facings'), { target: { value: '4' } });
    expect(screen.queryByRole('alert')).toBeNull();
    onPreview.mockReturnValue(undefined);
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(onPreview).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole('alert')).toBeNull();
  });

  it('keeps the locked baseline unavailable with an explanation', () => {
    const { onPreview } = renderForm({ context: { ...context, version_status: 'published' } });
    expect((screen.getByRole('button', { name: 'Preview sales allocation' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('status').textContent).toContain('baseline is locked');
    fireEvent.submit(screen.getByRole('button', { name: 'Preview sales allocation' }).closest('form')!);
    expect(onPreview).not.toHaveBeenCalled();
  });

  it('permits an editable proposed version', () => {
    const { onPreview } = renderForm({ context: { ...context, version_status: 'proposed' } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(onPreview).toHaveBeenCalledOnce();
  });

  it('lets an empty selected shelf use the bay’s occupied adjustable shelves', () => {
    const { onPreview } = renderForm({ shelf: secondShelf });
    expect((screen.getByRole('button', { name: 'Preview sales allocation' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('status').textContent).toContain('This shelf is empty');
    fireEvent.change(screen.getByLabelText('Allocation scope'), { target: { value: 'bay' } });
    fireEvent.click(screen.getByRole('button', { name: 'Preview sales allocation' }));
    expect(onPreview).toHaveBeenCalledWith(expect.objectContaining({ scope: { kind: 'bay', section_id: 'section_01' } }));
  });

  it('explains a tray-only scope instead of pretending tray presets can change', () => {
    renderForm({ context: { ...context, placements: [trayPlacement()] } });
    expect((screen.getByRole('button', { name: 'Preview sales allocation' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('status').textContent).toContain('only loaded trays');
  });

  it('keeps the fixed base deck unavailable', () => {
    renderForm({ shelf: baseDeck });
    expect((screen.getByRole('button', { name: 'Preview sales allocation' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByRole('status').textContent).toContain('Select an adjustable shelf');
  });
});

describe('Sales allocation review', () => {
  it('displays the Rust report shares, counts, synthetic provenance and constraints', () => {
    const report = readySalesAllocationPreview(0).sales_allocation!;
    render(<SalesAllocationReview report={report} products={[looseProduct, trayProduct]}/>);
    const table = screen.getByRole('table', { name: 'Sales allocation by product' });
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3);
    expect(rows[1].textContent).toContain('75.0%3 facings');
    expect(rows[2].textContent).toContain('25.0%1 facing');
    expect(rows[2].textContent).toContain('50.0%3 facings');
    expect(screen.getByText(report.source)).toBeDefined();
    expect(screen.getByText(/Contribution counts each SKU once/).textContent).toContain('including trays with fixed facings');
    expect(screen.getByRole('list', { name: 'Allocation constraints' }).textContent).toContain(report.warnings[0]);
    expect(screen.getByText(/This deterministic allocation/).textContent).toContain('synthetic demand stays unchanged');
  });

  it('filters a large report without changing reported values or repeating a brand prefix', () => {
    const report = readySalesAllocationPreview(0).sales_allocation!;
    const products = Array.from({ length: 12 }, (_, index) => ({ ...looseProduct, id: `cereal_${index}`, brand: 'Oakbrook', description: `Oakbrook Cereal ${index}` }));
    render(<SalesAllocationReview products={products} report={{ ...report, target: 'space', product_count: 12, rows: products.map(product => ({ ...report.rows[0], product_id: product.id })) }}/>);
    const table = screen.getByRole('table', { name: 'Sales allocation by product' });
    expect(within(table).getAllByRole('row')).toHaveLength(13);
    fireEvent.change(screen.getByLabelText('Filter allocation products'), { target: { value: 'cereal_11' } });
    expect(within(table).getAllByRole('row')).toHaveLength(2);
    expect(within(table).getByRole('rowheader').textContent).toContain('Oakbrook Cereal 11');
    expect(within(table).getByRole('rowheader').textContent).not.toContain('Oakbrook Oakbrook');
    expect(screen.getByRole('status').textContent).toBe('Showing 1 of 12 products.');
    expect(screen.getByText(/Contribution counts each SKU once/).textContent).toContain('exclude gaps');
  });
});
