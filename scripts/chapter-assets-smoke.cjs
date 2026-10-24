// Run only against disposable PostgreSQL/S3 project data.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const { randomUUID } = require('node:crypto');
const fs = require('node:fs');
const baseURL = process.env.COMIC_TEST_URL || 'http://127.0.0.1:8080';
const password = process.env.COMIC_TEST_PASSWORD || '';

(async () => {
  const browser = await chromium.launch({ executablePath: process.env.COMIC_CHROMIUM === 'bundled' ? undefined : (process.env.COMIC_CHROMIUM || '/usr/bin/chromium'), headless: true, args: ['--no-sandbox'] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, httpCredentials: { username: 'comic', password, send: 'always' }, extraHTTPHeaders: { Origin: new URL(baseURL).origin } });
  const page = await context.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  const button = name => page.getByRole('button', { name, exact: true });
  const saved = () => page.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
  const read = async id => {
    const response = await context.request.get(`${baseURL}/api/projects/${id}`);
    assert(response.ok(), `Read project: ${response.status()}`);
    return response.json();
  };
  const waitData = async (id, predicate) => {
    for (let i = 0; i < 100; i++) {
      const snapshot = await read(id);
      if (predicate(snapshot.workspace)) { await saved(); return snapshot; }
      await page.waitForTimeout(100);
    }
    throw new Error('Timed out waiting for chapter/asset persistence');
  };
  const view = label => page.getByRole('navigation', { name: 'Workspace views' }).getByRole('button', { name: label }).click();
  const createProject = async name => {
    await button('New project').click();
    await page.getByLabel('New project name', { exact: true }).fill(name);
    await button('Create project').click();
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
    return new URL(page.url()).searchParams.get('project');
  };
  const library = () => page.getByRole('main', { name: 'Asset library', exact: true });
  const composition = () => view('Page composition');
  const assetView = () => view('Asset library');
  const selectPanel = index => page.getByRole('navigation', { name: 'Chapter panels' }).getByRole('button').nth(index).click();
  const createAsset = async (kind, name) => {
    await page.getByLabel('Asset type', { exact: true }).selectOption(kind);
    await button(`Create ${kind.toLowerCase()}`).click();
    await page.getByLabel('Asset name', { exact: true }).fill(name);
    await page.getByLabel('Asset description', { exact: true }).fill(`${name} description`);
    await page.getByLabel('Visual design notes', { exact: true }).fill(`${name} design`);
    if (kind === 'Character') await page.getByLabel('Character size', { exact: true }).fill('Small, three feet');
    else assert.equal(await page.getByLabel('Character size', { exact: true }).count(), 0);
  };
  try {
    const suffix = randomUUID().slice(0, 8);
    const name = `Chapter assets ${suffix}`;
    await page.goto(`${baseURL}/?view=projects`);
    const id = await createProject(name);
    await page.getByLabel('Chapter title', { exact: true }).fill('Retained chapter');
    await page.getByLabel('Chapter description', { exact: true }).fill('Retained chapter description');
    await waitData(id, w => w.chapters[0].title === 'Retained chapter');
    await button('New chapter').click();
    await page.getByLabel('Chapter title', { exact: true }).fill('Five panel chapter');
    await page.getByLabel('Chapter description', { exact: true }).fill('Disposable five panel chapter');
    for (let i = 0; i < 5; i++) await button('Add panel').click();
    await waitData(id, w => w.chapters[1]?.panel_ids.length === 5);
    await button('Move chapter Five panel chapter up').click();
    await waitData(id, w => w.chapters[0].title === 'Five panel chapter');
    await page.reload(); await saved();
    assert.equal(await page.getByRole('region', { name: 'Chapter management' }).getByRole('listitem').first().getByRole('button').first().getAttribute('aria-label'), 'Open chapter Five panel chapter');
    await button('Open chapter Five panel chapter').click();
    assert.equal(await page.getByLabel('Chapter description', { exact: true }).inputValue(), 'Disposable five panel chapter');

    await assetView();
    await createAsset('Character', 'Moss');
    await createAsset('Location', 'Old tower');
    await createAsset('Equipment', 'Silver bow');
    let snapshot = await waitData(id, w => w.assets.length === 3 && w.assets.some(a => a.name === 'Silver bow'));
    assert.deepEqual(snapshot.workspace.assets.map(a => [a.kind, a.name, a.description, a.design_notes]), [
      ['Character', 'Moss', 'Moss description', 'Moss design'],
      ['Location', 'Old tower', 'Old tower description', 'Old tower design'],
      ['Equipment', 'Silver bow', 'Silver bow description', 'Silver bow design'],
    ]);
    await page.reload(); await saved();
    await assetView();
    const character = snapshot.workspace.assets.find(a => a.name === 'Moss');
    const location = snapshot.workspace.assets.find(a => a.name === 'Old tower');
    await page.getByLabel('Asset type', { exact: true }).selectOption('Character');
    await page.getByLabel('Search assets', { exact: true }).fill('MOs');
    await button('Moss').click();
    assert.equal(await page.getByLabel('Character size', { exact: true }).inputValue(), 'Small, three feet');
    await page.getByLabel('Search assets', { exact: true }).fill('absent character');
    assert.equal(await page.getByRole('navigation', { name: 'Project assets' }).getByRole('button').count(), 0);
    await page.getByLabel('Search assets', { exact: true }).fill('');
    await composition();
    await button('Open chapter Retained chapter').click();
    for (const index of [0, 1]) {
      await selectPanel(index);
      await page.getByRole('checkbox', { name: 'Moss', exact: true }).check();
    }
    await page.getByRole('checkbox', { name: 'Silver bow', exact: true }).check();
    await page.getByLabel('Panel location', { exact: true }).selectOption(location.id);
    await waitData(id, w => w.panels.filter(p => p.assets.characters.includes(character.id)).length === 2 && w.panels.some(p => p.assets.location === location.id && p.assets.equipment.length === 1));
    await assetView();
    await page.getByLabel('Asset type', { exact: true }).selectOption('Character');
    await button('Moss').click();
    assert(await button('Delete asset').isDisabled());
    assert.equal(await library().locator('.asset-usage li').count(), 2);
    await page.getByLabel('Asset description', { exact: true }).fill('Shared updated description');
    await waitData(id, w => w.assets.find(a => a.id === character.id).description === 'Shared updated description');
    await composition();
    for (const index of [0, 1]) {
      await selectPanel(index);
      const definition = page.locator('details').filter({ has: page.getByText('Moss — shared definition', { exact: true }) });
      if (!await definition.getAttribute('open').then(value => value !== null)) await definition.locator('summary').click();
      assert(await page.getByText('Shared updated description', { exact: true }).isVisible());
      await page.getByRole('checkbox', { name: 'Moss', exact: true }).uncheck();
      if (index === 0) await waitData(id, w => w.panels.filter(p => p.assets.characters.includes(character.id)).length === 1);
    }
    await waitData(id, w => w.panels.every(p => !p.assets.characters.includes(character.id)));
    await assetView();
    await button('Moss').click();
    await button('Delete asset').click();
    await button('Cancel asset deletion').click();
    assert.equal((await read(id)).workspace.assets.length, 3);
    await button('Delete asset').click();
    await button('Confirm delete asset').click();
    await waitData(id, w => w.assets.length === 2);

    await composition();
    await button('Open chapter Five panel chapter').click();
    await selectPanel(0);
    await page.getByRole('checkbox', { name: 'Silver bow', exact: true }).check();
    await page.getByLabel('Panel location', { exact: true }).selectOption(location.id);
    await page.getByLabel('Panel expression', { exact: true }).fill('Determined');
    await page.getByLabel('Action & composition', { exact: true }).fill('The archer approaches the tower.');
    await waitData(id, w => w.panels.some(p => p.expression === 'Determined' && p.action === 'The archer approaches the tower.' && p.assets.location === location.id && p.assets.equipment.length === 1));
    await page.reload(); await saved();
    await button('Open chapter Five panel chapter').click();
    await selectPanel(0);
    assert.equal(await page.getByLabel('Panel expression', { exact: true }).inputValue(), 'Determined');
    assert.equal(await page.getByLabel('Action & composition', { exact: true }).inputValue(), 'The archer approaches the tower.');
    const beforeDelete = (await read(id)).workspace;
    await button('Delete chapter').click();
    const dialog = page.getByRole('dialog', { name: 'Delete chapter confirmation' });
    assert.match(await dialog.innerText(), /delete 5 panels/);
    await button('Cancel chapter deletion').click();
    assert.deepEqual((await read(id)).workspace, beforeDelete);
    await button('Delete chapter').click();
    await button('Confirm chapter deletion').click();
    snapshot = await waitData(id, w => w.chapters.length === 1 && w.panels.length === 4);
    assert.deepEqual(snapshot.workspace.assets, beforeDelete.assets);
    assert.deepEqual(snapshot.workspace.chapters[0], beforeDelete.chapters.find(c => c.title === 'Retained chapter'));
    assert.deepEqual(snapshot.workspace.panels, beforeDelete.panels.filter(p => snapshot.workspace.chapters[0].panel_ids.includes(p.id)));
    await page.reload(); await saved();
    assert.equal(await page.getByLabel('Chapter description', { exact: true }).inputValue(), 'Retained chapter description');
    await assetView();
    await page.getByLabel('Asset type', { exact: true }).selectOption('Location');
    await button('Old tower').click();
    assert.equal(await page.getByLabel('Asset description', { exact: true }).inputValue(), 'Old tower description');

    await page.setViewportSize({ width: 390, height: 844 });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone asset library overflows');
    fs.mkdirSync('artifacts', { recursive: true });
    await page.screenshot({ path: 'artifacts/chapter-assets-phone.png', fullPage: true });
    await composition();
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone chapter editor overflows');
    await button('Projects').click();
    const other = await createProject(`Isolated chapter assets ${suffix}`);
    assert.equal((await read(other)).workspace.assets.length, 0);
    await assetView();
    assert.equal(await page.getByRole('navigation', { name: 'Project assets' }).getByRole('button').count(), 0);
    await composition();
    await button('Delete chapter').click();
    await button('Confirm chapter deletion').click();
    await waitData(other, w => w.chapters.length === 0 && w.panels.length === 0);
    assert(await page.getByText('No chapters yet. Create a chapter to start planning your story.', { exact: true }).isVisible());
    await page.reload(); await saved();
    await button('New chapter').click();
    await waitData(other, w => w.chapters.length === 1);
    assert.deepEqual((await read(id)).workspace, snapshot.workspace, 'Other project edits must preserve first project');
    assert.deepEqual(errors, [], 'Browser runtime errors');
    console.log('PASS: chapter create/edit/reorder/reopen, five-panel cancel/delete, retained assets, all asset types/search/size/shared definitions/associations/deletion guard, project isolation, empty state and phone views');
  } catch (error) {
    fs.mkdirSync('artifacts', { recursive: true });
    console.error('Chapter/assets browser errors:', errors);
    console.error('Chapter/assets browser state:', await page.locator('body').innerText().catch(() => 'Unavailable'));
    await page.screenshot({ path: 'artifacts/chapter-assets-failure.png', fullPage: true }).catch(() => {});
    throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
