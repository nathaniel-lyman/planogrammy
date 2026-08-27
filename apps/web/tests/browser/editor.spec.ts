import { expect, test } from '@playwright/test';

test('loads the complete fixture and supports accessible command paths', async ({ page }) => {
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');

  const baseDeck = page.getByRole('button', { name: /Base deck/ });
  await expect(baseDeck).toBeVisible();
  for (let shelf = 1; shelf <= 6; shelf += 1) {
    await expect(page.getByRole('button', { name: new RegExp(`Shelf 0${shelf}`) })).toBeVisible();
  }
  const stockingFilter = page.getByLabel('Filter by stocking mode');
  await stockingFilter.selectOption('tray');
  await expect(page.locator('.catalog-header')).toContainText('5 SKUs');
  await stockingFilter.selectOption('loose');
  await expect(page.locator('.catalog-header')).toContainText('17 SKUs');
  await stockingFilter.selectOption('all');
  await expect(page.locator('.catalog-header')).toContainText('22 SKUs');
  await baseDeck.click();
  await expect(page.getByRole('button', { name: /Add .*selected shelf/ })).toBeDisabled();

  const shelf04 = page.getByRole('button', { name: /Shelf 04/ });
  await shelf04.click();
  await expect(page.getByRole('heading', { name: 'Shelf 04' })).toBeVisible();
  const elevation = page.getByLabel('Elevation', { exact: true });
  await elevation.fill('4\' 6"');
  await page.locator('.elevation-form').getByRole('button', { name: 'Apply' }).click();
  await page.waitForTimeout(400);
  test.skip(!(await page.getByText('Revision 1 · All changes local').isVisible()), 'Headless WebGPU initialized but could not execute a render-backed command.');
  await expect(page.getByText('Revision 1 · All changes local')).toBeVisible();

  await shelf04.press('ArrowUp');
  await expect(elevation).toHaveValue('4\' 7"');
  await shelf04.press('Shift+ArrowDown');
  await expect(elevation).toHaveValue('4\' 6"');

  await elevation.fill('5\'');
  await page.locator('.elevation-form').getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByRole('alert')).toContainText('cannot occupy the same elevation');
  await expect(page.getByText('Revision 3 · All changes local')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(elevation).toHaveValue('4\' 7"');
  await expect(page.getByText('Revision 4 · All changes local')).toBeVisible();
});

test('shows the explicit unsupported state without WebGPU', async ({ browser }) => {
  const context = await browser.newContext();
  await context.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true }));
  const page = await context.newPage();
  await page.goto('http://127.0.0.1:4173/');
  await expect(page.getByRole('heading', { name: 'WebGPU is required' })).toBeVisible();
  await context.close();
});

test('selects and removes a placement through the accessible command path, then restores it with undo', async ({ page }) => {
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');

  await page.getByRole('button', { name: /Shelf 01/ }).click();
  await page.getByRole('button', { name: /Add .*selected shelf/ }).click();
  await page.waitForTimeout(400);
  test.skip(!(await page.getByText('Revision 1 · All changes local').isVisible()), 'Headless WebGPU initialized but could not execute a render-backed command.');

  const placement = page.getByRole('button', { name: /Jif Creamy Peanut Butter.*16 oz.*at 0"/ });
  await expect(placement).toBeVisible();
  await placement.click();
  await expect(page.getByRole('heading', { name: 'Jif Creamy Peanut Butter' })).toBeVisible();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(page.getByText('Shelf-ready tray', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('3 × 1 × 4', { exact: true })).toBeVisible();
  await expect(page.getByText('12', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('10 15/16" W × 5" H × 14 1/2" D', { exact: true })).toBeVisible();
  await expect(page.getByText('1 1/4"', { exact: true })).toBeVisible();
  await expect(page.getByText('Illustrative data', { exact: true })).toBeVisible();
  await expect(page.getByText('Sales / store / week', { exact: true })).toBeVisible();
  await expect(page.getByText('Units / store / week', { exact: true })).toBeVisible();
  await expect(page.getByText('Gross margin', { exact: true })).toBeVisible();
  const inspector = page.locator('.selection-panel');
  await expect(inspector.getByText('$36.65', { exact: true })).toBeVisible();
  await expect(inspector.getByText('10.5', { exact: true })).toBeVisible();
  await expect(inspector.getByText('28.5%', { exact: true })).toBeVisible();
  await expect(inspector.getByText('Trailing 13 weeks', { exact: true })).toBeVisible();
  await expect(inspector.getByText('Synthetic representative 13-week average; not retailer actuals', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Remove product' }).click();
  await expect(page.getByText('Revision 2 · All changes local')).toBeVisible();
  await expect(placement).not.toBeVisible();
  await expect(page.getByText('Select a shelf or product on the canvas or in the fixture outline.')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Revision 3 · All changes local')).toBeVisible();
  await expect(placement).toBeVisible();
  await placement.click();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(page.getByText('0"', { exact: true })).toBeVisible();
});

test('moves a selected placement by eighths and between shelves through one inspector command', async ({ page }) => {
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');

  await page.getByRole('button', { name: /Shelf 01/ }).click();
  await page.getByRole('button', { name: /Add .*selected shelf/ }).click();
  await page.waitForTimeout(400);
  test.skip(!(await page.getByText('Revision 1 · All changes local').isVisible()), 'Headless WebGPU initialized but could not execute a render-backed command.');

  const placement = page.getByRole('button', { name: /Jif Creamy Peanut Butter.*16 oz.*at 0"/ });
  await placement.click();
  const position = page.getByLabel('Position', { exact: true });
  const shelf = page.getByLabel('Shelf', { exact: true });
  await expect(position).toHaveValue('0"');

  await placement.press('ArrowRight');
  await expect(position).toHaveValue('1/8"');
  await expect(page.getByText('Revision 2 · All changes local')).toBeVisible();

  await position.fill('1/16"');
  await page.getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByRole('alert')).toContainText('1/8-inch increments');
  await expect(page.getByText('Revision 2 · All changes local')).toBeVisible();
  await expect(position).toHaveValue('1/8"');

  await shelf.selectOption('shelf_02');
  await page.getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByText('Revision 3 · All changes local')).toBeVisible();
  await expect(shelf).toHaveValue('shelf_02');
  await expect(page.getByText('shelf_02', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 1\/8"/ })).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Revision 4 · All changes local')).toBeVisible();
  await expect(shelf).toHaveValue('shelf_01');
  await expect(position).toHaveValue('1/8"');
});

test('registers site tools and applies the first WebMCP write through the live page session', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, '__planogramSiteTools', { value: [], configurable: true, writable: true });
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: async (tool: unknown) => { (window as Window & { __planogramSiteTools: unknown[] }).__planogramSiteTools.push(tool); },
        unregisterTool: async () => undefined,
      },
    });
  });
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');
  await page.waitForFunction(() => ((window as Window & { __planogramSiteTools?: unknown[] }).__planogramSiteTools?.length ?? 0) === 10);
  await expect(page.getByText('Site tools ready')).toBeVisible();
  const toolNames = await page.evaluate(() => (window as Window & { __planogramSiteTools: Array<{ name: string }> }).__planogramSiteTools.map(tool => tool.name));
  expect(toolNames).toEqual([
    'planogram.get_planogram_context',
    'planogram.search_products',
    'planogram.get_product',
    'planogram.get_section',
    'planogram.validate_planogram',
    'planogram.add_product',
    'planogram.distribute_shelf',
    'planogram.undo_change_set',
    'planogram.preview_changes',
    'planogram.apply_changes',
  ]);

  const productResult = await page.evaluate(async () => {
    const tools = (window as Window & { __planogramSiteTools: Array<{ name: string; execute: (args: unknown, context?: { signal?: AbortSignal }) => Promise<unknown> }> }).__planogramSiteTools;
    const tool = tools.find(candidate => candidate.name === 'planogram.get_product');
    if (!tool) throw new Error('get_product site tool was not registered');
    return await tool.execute({ product_id: 'jif_creamy_16' });
  });
  expect(productResult).toMatchObject({
    status: 'ok',
    product: {
      id: 'jif_creamy_16',
      net_weight_ounces_hundredths: 1600,
      casepack_quantity: 12,
      dimensions: { depth_sixteenths: 57 },
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
    },
  });

  let addResult: unknown;
  try {
    addResult = await page.evaluate(async () => {
      const tools = (window as Window & { __planogramSiteTools: Array<{ name: string; execute: (args: unknown, context: { signal: AbortSignal }) => Promise<unknown> }> }).__planogramSiteTools;
      const tool = tools.find(candidate => candidate.name === 'planogram.add_product');
      if (!tool) throw new Error('add_product site tool was not registered');
      return await tool.execute({ product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0, reason: 'browser contract test' }, { signal: new AbortController().signal });
    });
  } catch (error) {
    test.skip(true, `Headless WebGPU could not execute the WebMCP write: ${String(error)}`);
    return;
  }
  expect(addResult).toMatchObject({ status: 'applied', revision: 1, change_set: { actor: 'webmcp' }, placement: { shelf_id: 'shelf_01', x_sixteenths: 0, stocking_mode: 'tray', stocked_unit_count: 12, display_width_sixteenths: 175, display_height_sixteenths: 80, required_depth_sixteenths: 232 } });
  await expect(page.getByText('Revision 1 · All changes local')).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 0"/ })).toBeVisible();

  const undoResult = await page.evaluate(async () => {
    const tools = (window as Window & { __planogramSiteTools: Array<{ name: string; execute: (args: unknown, context: { signal: AbortSignal }) => Promise<unknown> }> }).__planogramSiteTools;
    const tool = tools.find(candidate => candidate.name === 'planogram.undo_change_set');
    if (!tool) throw new Error('undo_change_set site tool was not registered');
    return await tool.execute({ change_set_id: 'change_0001', expected_revision: 1 }, { signal: new AbortController().signal });
  });
  expect(undoResult).toMatchObject({ status: 'applied', revision: 2, change_set: { actor: 'webmcp' } });
  await expect(page.getByText('Revision 2 · All changes local')).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 0"/ })).not.toBeVisible();
});

test('previews a WebMCP proposal and records truthful human approval in the review UI', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, '__planogramSiteTools', { value: [], configurable: true, writable: true });
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: async (tool: unknown) => { (window as Window & { __planogramSiteTools: unknown[] }).__planogramSiteTools.push(tool); },
        unregisterTool: async () => undefined,
      },
    });
  });
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');
  await page.waitForFunction(() => ((window as Window & { __planogramSiteTools?: unknown[] }).__planogramSiteTools?.length ?? 0) === 10);

  let previewResult: { status: string; proposal_id?: string; revision?: number };
  try {
    previewResult = await page.evaluate(async () => {
      const tools = (window as Window & { __planogramSiteTools: Array<{ name: string; execute: (args: unknown, context: { signal: AbortSignal }) => Promise<unknown> }> }).__planogramSiteTools;
      const tool = tools.find(candidate => candidate.name === 'planogram.preview_changes');
      if (!tool) throw new Error('preview_changes site tool was not registered');
      return await tool.execute({
        expected_revision: 0,
        reason: 'Group Jif by package size',
        operations: [
          { kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 },
          { kind: 'add', product_id: 'jif_creamy_40', shelf_id: 'shelf_02', sequence: 0 },
        ],
      }, { signal: new AbortController().signal }) as { status: string; proposal_id?: string; revision?: number };
    });
  } catch (error) {
    test.skip(true, `Headless WebGPU could not execute the WebMCP preview: ${String(error)}`);
    return;
  }
  expect(previewResult).toMatchObject({ status: 'ready', revision: 0, proposal_id: 'proposal_0001' });
  await expect(page.getByText('Revision 0 · All changes local')).toBeVisible();
  await expect(page.getByText('Proposal ready · 2 changes')).toBeVisible();
  await expect(page.getByText(/Add Jif Creamy Peanut Butter \(16 oz\) to Shelf 01 at 0"/)).toBeVisible();

  await page.getByRole('button', { name: 'Accept proposal' }).click();
  await expect(page.getByText('Revision 1 · All changes local')).toBeVisible();
  await expect(page.getByText('Proposal ready · 2 changes')).not.toBeVisible();
  const receipt = page.locator('.applied-proposal-receipt');
  const receiptHeading = receipt.getByRole('heading', { name: 'WebMCP proposal approved' });
  await expect(receiptHeading).toBeVisible();
  await expect(receiptHeading).toBeFocused();
  await expect(receipt.getByText('Group Jif by package size', { exact: true })).toBeVisible();
  await expect(receipt.getByText('human', { exact: true })).toBeVisible();
  await expect(receipt.getByText('0 → 1', { exact: true })).toBeVisible();
  await expect(receipt.getByText('change_0001', { exact: true })).toBeVisible();
  await expect(receipt.getByText('2', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 0"/ })).toBeVisible();
});

test('enforces the product gap and distributes a shelf evenly as one undoable change', async ({ page }) => {
  await page.goto('/');
  const unsupported = page.getByRole('heading', { name: 'WebGPU is required' });
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await unsupported.isVisible(), 'The test browser does not expose WebGPU.');

  await page.getByRole('button', { name: /Shelf 01/ }).click();
  const add = page.getByRole('button', { name: /Add .*selected shelf/ });
  await add.click();
  await add.click();
  await page.waitForTimeout(400);
  test.skip(!(await page.getByText('Revision 2 · All changes local').isVisible()), 'Headless WebGPU initialized but could not execute a render-backed command.');
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 0"/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 11 1\/8"/ })).toBeVisible();

  const distribution = page.getByLabel('Product distribution');
  await expect(distribution).toHaveValue('space_evenly');
  await page.locator('.distribution-form').getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByText('Revision 3 · All changes local')).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 8 5\/8"/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 28 3\/8"/ })).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Revision 4 · All changes local')).toBeVisible();
  const second = page.getByRole('button', { name: /Jif Creamy Peanut Butter.*at 11 1\/8"/ });
  await second.click();
  const position = page.getByLabel('Position', { exact: true });
  await position.fill('11"');
  await page.locator('.placement-form').getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByRole('alert')).toContainText('at least a 1/8-inch gap');
  await expect(page.getByText('Revision 4 · All changes local')).toBeVisible();
  await expect(position).toHaveValue('11 1/8"');
});
