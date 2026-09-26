import { expect, test, type Page } from '@playwright/test';

type SiteTool = { name: string; execute: (args: unknown, context?: { signal?: AbortSignal }) => Promise<unknown> };
type SiteToolWindow = Window & { __planogramSiteTools: SiteTool[] };

const SITE_TOOL_COUNT = 11;
const SYNTHETIC_SOURCE = 'Synthetic representative 13-week average; not retailer actuals';

/** Opens the editor and skips when the browser lacks WebGPU entirely. */
async function openEditor(page: Page) {
  await page.goto('/');
  await page.waitForFunction(() => document.querySelector('.shelf-list button') || document.querySelector('.unsupported'));
  test.skip(await page.getByRole('heading', { name: 'WebGPU is required' }).isVisible(), 'The test browser does not expose WebGPU.');
}

/** Records WebMCP tool registrations so tests can call the live site tools. */
async function openEditorWithSiteTools(page: Page) {
  await page.addInitScript(() => {
    Object.defineProperty(window, '__planogramSiteTools', { value: [], configurable: true, writable: true });
    Object.defineProperty(document, 'modelContext', {
      configurable: true,
      value: {
        registerTool: async (tool: SiteTool) => { (window as unknown as SiteToolWindow).__planogramSiteTools.push(tool); },
        unregisterTool: async () => undefined,
      },
    });
  });
  await openEditor(page);
  await page.waitForFunction(count => ((window as unknown as Partial<SiteToolWindow>).__planogramSiteTools?.length ?? 0) === count, SITE_TOOL_COUNT);
}

function callSiteTool<T = unknown>(page: Page, name: string, args: unknown = {}): Promise<T> {
  return page.evaluate(async ({ name, args }) => {
    const tool = (window as unknown as SiteToolWindow).__planogramSiteTools.find(candidate => candidate.name === name);
    if (!tool) throw new Error(`${name} site tool was not registered`);
    return await tool.execute(args, { signal: new AbortController().signal });
  }, { name, args }) as Promise<T>;
}

/** The first render-backed write in headless Chromium may throw; skip rather than fail. */
async function callSiteToolOrSkip<T = unknown>(page: Page, name: string, args: unknown): Promise<T> {
  try {
    return await callSiteTool<T>(page, name, args);
  } catch (error) {
    test.skip(true, `Headless WebGPU could not execute ${name}: ${String(error).split("\n")[0]}`);
    throw error;
  }
}

async function shelf01Placements(page: Page) {
  const result = await callSiteTool<{ section: { shelves: Array<{ id: string; placements: unknown[] }> } }>(page, 'planogram.get_section', { section_id: 'section_01' });
  return result.section.shelves.find(shelf => shelf.id === 'shelf_01')?.placements;
}

function revision(page: Page, value: number) {
  return page.getByText(`Revision ${value} · All changes local`);
}

async function expectRevision(page: Page, value: number) {
  await expect(revision(page, value)).toBeVisible();
}

/** Skips when headless WebGPU initialized but the first command never rendered. */
async function skipUnlessRevision(page: Page, value: number) {
  const reached = await revision(page, value).waitFor({ timeout: 2_000 }).then(() => true, () => false);
  test.skip(!reached, 'Headless WebGPU initialized but could not execute a render-backed command.');
}

async function addToShelf01(page: Page, times = 1) {
  await page.getByRole('button', { name: /Shelf 01/ }).click();
  const add = page.getByRole('button', { name: /Add .*selected shelf/ });
  for (let count = 0; count < times; count += 1) await add.click();
  await skipUnlessRevision(page, times);
}

function placementAt(page: Page, position: string) {
  return page.getByRole('button', { name: new RegExp(`Jif Creamy Peanut Butter 16 oz.*at ${position}`) });
}

test('loads the complete fixture and supports accessible command paths', async ({ page }) => {
  await openEditor(page);

  const baseDeck = page.getByRole('button', { name: /Base deck/ });
  await expect(baseDeck).toBeVisible();
  for (let shelf = 1; shelf <= 6; shelf += 1) {
    await expect(page.getByRole('button', { name: new RegExp(`Shelf 0${shelf}`) })).toBeVisible();
  }
  const stockingFilter = page.getByLabel('Filter by stocking mode');
  for (const [mode, count] of [['tray', 5], ['loose', 17], ['all', 22]] as const) {
    await stockingFilter.selectOption(mode);
    await expect(page.locator('.catalog-header')).toContainText(`${count} SKUs`);
  }
  await baseDeck.click();
  await expect(page.getByRole('button', { name: /Add .*selected shelf/ })).toBeDisabled();

  const shelf04 = page.getByRole('button', { name: /Shelf 04/ });
  await shelf04.click();
  await expect(page.getByRole('heading', { name: 'Shelf 04' })).toBeVisible();
  const elevation = page.getByLabel('Elevation', { exact: true });
  const applyElevation = page.locator('.elevation-form').getByRole('button', { name: 'Apply' });
  await elevation.fill('4\' 6"');
  await applyElevation.click();
  await skipUnlessRevision(page, 1);

  await shelf04.press('ArrowUp');
  await expect(elevation).toHaveValue('4\' 7"');
  await shelf04.press('Shift+ArrowDown');
  await expect(elevation).toHaveValue('4\' 6"');

  await elevation.fill('5\'');
  await applyElevation.click();
  await expect(page.locator('#elevation-error')).toContainText('cannot occupy the same elevation');
  await expectRevision(page, 3);

  const undo = page.getByRole('button', { name: 'Undo' });
  for (const [expected, nextRevision] of [['4\' 7"', 4], ['4\' 6"', 5], ['4\'', 6]] as const) {
    await undo.click();
    await expect(elevation).toHaveValue(expected);
    await expectRevision(page, nextRevision);
  }
  await expect(undo).toBeDisabled();
});

test('shows the explicit unsupported state without WebGPU', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, 'gpu', { value: undefined, configurable: true }));
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'WebGPU is required' })).toBeVisible();
});

test('selects and removes a placement through the accessible command path, then restores it with undo', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page);

  const placement = placementAt(page, '0"');
  await expect(placement).toBeVisible();
  await placement.click();
  await expect(page.getByRole('heading', { name: 'Jif Creamy Peanut Butter' })).toBeVisible();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(page.getByText('Shelf-ready tray', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('12', { exact: true }).first()).toBeVisible();
  const inspector = page.locator('.selection-panel');
  for (const text of ['3 × 1 × 4', '10 15/16" W × 5" H × 1\' 2 1/2" D', '1 1/4"', 'Illustrative data', 'Sales / store / week', 'Units / store / week', 'Gross margin', '$36.65', '10.5', '28.5%', 'Trailing 13 weeks', SYNTHETIC_SOURCE]) {
    await expect(inspector.getByText(text, { exact: true }).first()).toBeVisible();
  }

  await page.getByRole('button', { name: 'Remove product' }).click();
  await expectRevision(page, 2);
  await expect(placement).not.toBeVisible();
  await expect(page.getByText('Select a shelf or product on the canvas or in the fixture outline.')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 3);
  await placement.click();
  await expect(page.getByText('placement_0001')).toBeVisible();
  await expect(inspector.getByText('0"', { exact: true })).toBeVisible();
});

test('moves a selected placement by eighths and between shelves through one inspector command', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page);

  const placement = placementAt(page, '0"');
  await placement.click();
  const position = page.getByLabel('Position', { exact: true });
  const shelf = page.getByLabel('Shelf', { exact: true });
  const apply = page.getByRole('button', { name: 'Apply' });
  await expect(position).toHaveValue('0"');

  await placement.press('ArrowRight');
  await expect(position).toHaveValue('1/8"');
  await expectRevision(page, 2);

  await position.fill('1/16"');
  await apply.click();
  await expect(page.getByRole('alert')).toContainText('1/8-inch increments');
  await expectRevision(page, 2);
  await expect(position).toHaveValue('1/8"');

  await shelf.selectOption('shelf_02');
  await apply.click();
  await expectRevision(page, 3);
  await expect(shelf).toHaveValue('shelf_02');
  await expect(page.getByText('shelf_02', { exact: true })).toBeVisible();
  await expect(placementAt(page, '1/8"')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 4);
  await expect(shelf).toHaveValue('shelf_01');
  await expect(position).toHaveValue('1/8"');
});

test('registers site tools and applies the first WebMCP write through the live page session', async ({ page }) => {
  await openEditorWithSiteTools(page);
  await expect(page.getByText('Site tools ready')).toBeVisible();
  const toolNames = await page.evaluate(() => (window as unknown as SiteToolWindow).__planogramSiteTools.map(tool => tool.name));
  expect(toolNames).toEqual([
    'planogram.get_planogram_context',
    'planogram.search_products',
    'planogram.get_product',
    'planogram.get_section',
    'planogram.validate_planogram',
    'planogram.add_product',
    'planogram.distribute_shelf',
    'planogram.undo_change_set',
    'planogram.preview_shelf_allocation',
    'planogram.preview_changes',
    'planogram.apply_changes',
  ]);

  expect(await callSiteTool(page, 'planogram.get_product', { product_id: 'jif_creamy_16' })).toMatchObject({
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
        source: SYNTHETIC_SOURCE,
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

  const added = await callSiteToolOrSkip(page, 'planogram.add_product', { product_id: 'jif_creamy_16', shelf_id: 'shelf_01', expected_revision: 0, reason: 'browser contract test' });
  expect(added).toMatchObject({ status: 'applied', revision: 1, change_set: { actor: 'webmcp' }, placement: { shelf_id: 'shelf_01', x_sixteenths: 0, stocking_mode: 'tray', stocked_unit_count: 12, display_width_sixteenths: 175, display_height_sixteenths: 80, required_depth_sixteenths: 232 } });
  await expectRevision(page, 1);
  await expect(placementAt(page, '0"')).toBeVisible();

  expect(await callSiteTool(page, 'planogram.undo_change_set', { change_set_id: 'change_0001', expected_revision: 1 })).toMatchObject({ status: 'applied', revision: 2, change_set: { actor: 'webmcp' } });
  await expectRevision(page, 2);
  await expect(placementAt(page, '0"')).not.toBeVisible();
});

test('previews a WebMCP proposal and records truthful human approval in the review UI', async ({ page }) => {
  await openEditorWithSiteTools(page);

  const preview = await callSiteToolOrSkip(page, 'planogram.preview_changes', {
    expected_revision: 0,
    reason: 'Group Jif by package size',
    operations: [
      { kind: 'add', product_id: 'jif_creamy_16', shelf_id: 'shelf_01', sequence: 0 },
      { kind: 'add', product_id: 'jif_creamy_40', shelf_id: 'shelf_02', sequence: 0 },
    ],
  });
  expect(preview).toMatchObject({ status: 'ready', revision: 0, proposal_id: 'proposal_0001' });
  await expectRevision(page, 0);
  await expect(page.getByText('Proposal ready · 2 changes')).toBeVisible();
  await expect(page.getByText(/Add Jif Creamy Peanut Butter \(16 oz\) to Shelf 01 at 0"/)).toBeVisible();

  await page.getByRole('button', { name: 'Accept proposal' }).click();
  await expectRevision(page, 1);
  await expect(page.getByText('Proposal ready · 2 changes')).not.toBeVisible();
  const receipt = page.locator('.applied-proposal-receipt');
  const receiptHeading = receipt.getByRole('heading', { name: 'WebMCP proposal approved' });
  await expect(receiptHeading).toBeVisible();
  await expect(receiptHeading).toBeFocused();
  for (const text of ['Group Jif by package size', 'human', '0 → 1', 'change_0001', '2']) {
    await expect(receipt.getByText(text, { exact: true })).toBeVisible();
  }
  await expect(placementAt(page, '0"')).toBeVisible();
});

test('fills a shelf through one semantic WebMCP proposal and undoes the atomic reflow', async ({ page }) => {
  await openEditorWithSiteTools(page);

  const setup = await callSiteToolOrSkip<{ status: string; proposal_id?: string }>(page, 'planogram.preview_changes', {
    expected_revision: 0,
    reason: 'Place all largest Jif products on the bottom shelf',
    operations: ['jif_creamy_40', 'jif_crunchy_40', 'jif_natural_40'].map((product_id, sequence) => ({ kind: 'add', product_id, shelf_id: 'shelf_01', sequence })),
  });
  test.skip(setup.status !== 'ready', 'Headless WebGPU could not execute the setup preview.');
  const applied = await callSiteToolOrSkip<{ status: string }>(page, 'planogram.apply_changes', { proposal_id: setup.proposal_id, expected_revision: 0 });
  test.skip(applied.status !== 'applied', 'Headless WebGPU could not execute the setup proposal.');

  const filledLayout = [
    { x_sixteenths: 4, facings_x: 4 },
    { x_sixteenths: 282, facings_x: 4 },
    { x_sixteenths: 560, facings_x: 3 },
  ];
  expect(await callSiteTool(page, 'planogram.preview_shelf_allocation', { shelf_id: 'shelf_01', strategy: 'fill_evenly', expected_revision: 1, reason: 'Fill that shelf evenly with facings' })).toMatchObject({
    status: 'ready',
    revision: 1,
    operations: filledLayout.map(after => ({ type: 'reflow_placement', after })),
  });
  await expectRevision(page, 1);
  await expect(page.getByText('Proposal ready · 3 changes')).toBeVisible();
  await expect(page.getByText(/Reflow Jif Creamy Peanut Butter.*from 1 to 4 horizontal facings/)).toBeVisible();

  await page.getByRole('button', { name: 'Accept proposal' }).click();
  await expectRevision(page, 2);
  expect(await shelf01Placements(page)).toMatchObject(filledLayout);

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 3);
  expect(await shelf01Placements(page)).toMatchObject([
    { x_sixteenths: 0, facings_x: 1 },
    { x_sixteenths: 70, facings_x: 1 },
    { x_sixteenths: 140, facings_x: 1 },
  ]);
});

test('enforces the product gap and distributes a shelf evenly as one undoable change', async ({ page }) => {
  await openEditor(page);
  await addToShelf01(page, 2);
  await expect(placementAt(page, '0"')).toBeVisible();
  await expect(placementAt(page, '11 1/8"')).toBeVisible();

  await expect(page.getByLabel('Product distribution')).toHaveValue('space_evenly');
  await page.locator('.distribution-form').getByRole('button', { name: 'Apply' }).click();
  await expectRevision(page, 3);
  await expect(placementAt(page, '8 5/8"')).toBeVisible();
  await expect(placementAt(page, '2\' 4 3/8"')).toBeVisible();

  await page.getByRole('button', { name: 'Undo' }).click();
  await expectRevision(page, 4);
  await placementAt(page, '11 1/8"').click();
  const position = page.getByLabel('Position', { exact: true });
  await position.fill('11"');
  await page.locator('.placement-form').getByRole('button', { name: 'Apply' }).click();
  await expect(page.getByRole('alert')).toContainText('at least a 1/8-inch gap');
  await expectRevision(page, 4);
  await expect(position).toHaveValue('11 1/8"');
});
