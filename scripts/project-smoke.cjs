// Run only against disposable PostgreSQL/S3 project data.
// Run again with --verify-retention after replacing the application container.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createHash, randomUUID } = require('node:crypto');

const baseURL = process.env.COMIC_TEST_URL || 'http://127.0.0.1:8080';
const password = process.env.COMIC_TEST_PASSWORD || '';
const artifact = process.env.COMIC_PROJECT_SNAPSHOT || 'artifacts/projects-before.json';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const auth = `Basic ${Buffer.from(`comic:${password}`).toString('base64')}`;
const request = async path => {
  const response = await fetch(`${baseURL}${path}`, { headers: { Authorization: auth }, signal: AbortSignal.timeout(10_000) });
  assert(response.ok, `${path}: HTTP ${response.status}`);
  return response;
};

async function verifyRetention() {
  const before = JSON.parse(fs.readFileSync(artifact, 'utf8'));
  assert.deepEqual(await (await request('/api/projects')).json(), before.list);
  for (const project of before.projects) {
    assert.deepEqual(await (await request(`/api/projects/${project.project_id}`)).json(), project);
  }
  for (const image of before.images) {
    assert.equal(digest(Buffer.from(await (await request(image.url)).arrayBuffer())), image.digest);
  }
  console.log('PASS: fresh container retains all projects, revisions, alternatives and original image bytes');
}

async function browserWorkflow() {
  const browser = await chromium.launch({
    executablePath: process.env.COMIC_CHROMIUM === 'bundled' ? undefined : (process.env.COMIC_CHROMIUM || '/usr/bin/chromium'),
    headless: true,
    args: ['--no-sandbox'],
  });
  const context = await browser.newContext({
    viewport: { width: 1440, height: 1000 },
    httpCredentials: { username: 'comic', password, send: 'always' },
    extraHTTPHeaders: { Origin: new URL(baseURL).origin },
  });
  const page = await context.newPage();
  const errors = [];
  context.on('page', opened => opened.on('pageerror', error => errors.push(error.message)));
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => {
    if (message.type() === 'error') console.error('Project browser console:', message.text());
  });
  const suffix = randomUUID().slice(0, 8);
  const firstName = `Browser project A ${suffix}`;
  const secondName = `Browser project B ${suffix}`;
  const saved = async (target = page) => {
    await target.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
  };
  const read = async id => {
    const response = await context.request.get(`${baseURL}/api/projects/${id}`);
    assert(response.ok());
    return response.json();
  };
  const waitForData = async (id, predicate) => {
    for (let attempt = 0; attempt < 100; attempt++) {
      const snapshot = await read(id);
      if (predicate(snapshot)) return snapshot;
      await page.waitForTimeout(100);
    }
    throw new Error(`Timed out waiting for project ${id}`);
  };
  const projects = () => page.getByRole('button', { name: 'Projects', exact: true });
  const open = async name => {
    await page.getByRole('button', { name: `Open project ${name}`, exact: true }).click();
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
  };
  const create = async (name, description, loseFirstResponse = false) => {
    await page.getByRole('button', { name: 'New project', exact: true }).click();
    await page.getByLabel('New project name', { exact: true }).fill(name);
    await page.getByLabel('New project description', { exact: true }).fill(description);
    if (loseFirstResponse) {
      let dropped = false;
      await page.route('**/api/projects', async route => {
        if (route.request().method() !== 'POST' || dropped) return route.continue();
        dropped = true;
        const response = await route.fetch();
        assert(response.ok(), 'Creation must commit before simulating a lost response');
        await route.abort('failed');
      });
      await page.getByRole('button', { name: 'Create project', exact: true }).click();
      await page.getByRole('alert').waitFor();
      assert.match(await page.getByRole('alert').innerText(), /unknown|retry/i);
      await page.unroute('**/api/projects');
    }
    await page.getByRole('button', { name: 'Create project', exact: true }).click();
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
    const id = new URL(page.url()).searchParams.get('project');
    assert.match(id || '', /^[0-9a-f-]{36}$/i);
    return id;
  };
  const editor = () => page.getByRole('complementary', { name: 'Panel editor' });
  const dialog = () => page.getByRole('dialog', { name: 'Unsaved changes' });
  const failSave = async id => {
    await page.route(`**/api/projects/${id}`, route => {
      if (route.request().method() === 'PUT') return route.fulfill({ status: 503, body: 'Saving is temporarily unavailable.' });
      return route.continue();
    });
  };
  const unblockSave = id => page.unroute(`**/api/projects/${id}`);
  try {
    await page.goto(`${baseURL}/?view=projects`);
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    const initialFirstName = `${firstName} draft`;
    const first = await create(initialFirstName, 'A description survives reopening.', true);
    const afterRetry = await (await context.request.get(`${baseURL}/api/projects`)).json();
    assert.equal(afterRetry.filter(project => project.name === initialFirstName).length, 1, 'Retrying creation must not create a duplicate');
    assert((await read(first)).workspace.panels.every(panel => panel.action === '' && panel.dialogue === '' && panel.images.length === 0), 'A new project must contain blank planning panels');
    await page.getByLabel('Project title', { exact: true }).fill(firstName);
    await page.getByLabel('Project description', { exact: true }).fill('A revised description.');
    await editor().getByLabel('Action').fill('Only project A contains this action.');
    await waitForData(first, s => s.workspace.title === firstName && s.description === 'A revised description.' && s.workspace.panels[0].action === 'Only project A contains this action.');
    await saved();
    await page.reload();
    await saved();
    assert.equal(new URL(page.url()).searchParams.get('project'), first);
    assert.equal(await page.getByLabel('Project title', { exact: true }).inputValue(), firstName);
    assert.equal(await page.getByLabel('Project description', { exact: true }).inputValue(), 'A revised description.');

    // Distinct project identities must isolate even the same numbered panels.
    const picture = async color => Buffer.from(await page.evaluate(color => {
      const canvas = document.createElement('canvas');
      canvas.width = 8; canvas.height = 8;
      canvas.getContext('2d').fillStyle = color;
      canvas.getContext('2d').fillRect(0, 0, 8, 8);
      return canvas.toDataURL('image/png').split(',')[1];
    }, color), 'base64');
    const firstBytes = await picture('#255322');
    await editor().locator('input[type=file]').setInputFiles({ name: 'a.png', mimeType: 'image/png', buffer: firstBytes });
    await waitForData(first, s => s.workspace.panels[0].images.length === 1);
    await saved();
    const firstBefore = await read(first);
    await projects().click();
    const second = await create(secondName, 'Project B stays separate.');
    assert.notEqual(first, second);
    await editor().getByLabel('Action').fill('Only project B contains this action.');
    await waitForData(second, s => s.workspace.panels[0].action === 'Only project B contains this action.');
    await saved();
    const secondBytes = await picture('#854422');
    await editor().locator('input[type=file]').setInputFiles({ name: 'b.png', mimeType: 'image/png', buffer: secondBytes });
    await waitForData(second, s => s.workspace.panels[0].images.length === 1);
    await saved();
    assert.deepEqual(await read(first), firstBefore);
    assert.notEqual((await read(second)).workspace.panels[0].selected_image, firstBefore.workspace.panels[0].selected_image);
    await projects().click();
    // A failed destination load retains the list and retries that destination.
    await page.route(`**/api/projects/${first}`, route => {
      if (route.request().method() === 'GET') return route.fulfill({ status: 503, body: 'Project load is temporarily unavailable.' });
      return route.continue();
    });
    await page.getByRole('button', { name: `Open project ${firstName}`, exact: true }).click();
    await page.getByRole('alert').waitFor();
    assert(await page.getByRole('heading', { name: 'Your projects', exact: true }).isVisible());
    await page.unroute(`**/api/projects/${first}`);
    await page.getByRole('button', { name: 'Retry', exact: true }).click();
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
    assert.equal(new URL(page.url()).searchParams.get('project'), first);
    assert.equal(await editor().getByLabel('Action').inputValue(), 'Only project A contains this action.');

    // A failed autosave keeps the draft, guards navigation and supports retry.
    await failSave(first);
    await editor().getByLabel('Dialogue').fill('Retain this draft until it is saved.');
    await page.getByRole('alert').waitFor();
    await projects().click();
    await dialog().getByRole('button', { name: 'Stay', exact: true }).click();
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'Retain this draft until it is saved.');
    assert.equal(new URL(page.url()).searchParams.get('project'), first);
    await projects().click();
    await dialog().getByRole('button', { name: 'Save and continue', exact: true }).click();
    await dialog().getByRole('alert').waitFor();
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'Retain this draft until it is saved.');
    assert.equal((await read(first)).workspace.panels[0].dialogue, '');
    await unblockSave(first);
    // The navigation intent may still be displayed after a failed flush.
    if (!await dialog().isVisible()) await projects().click();
    await dialog().getByRole('button', { name: 'Save and continue', exact: true }).click();
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    assert.equal((await read(first)).workspace.panels[0].dialogue, 'Retain this draft until it is saved.');
    await open(firstName);

    await failSave(first);
    await editor().getByLabel('Dialogue').fill('Explicitly discarded draft.');
    await page.getByRole('alert').waitFor();
    await projects().click();
    await dialog().getByRole('button', { name: 'Discard changes and continue', exact: true }).click();
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    await unblockSave(first);
    await open(firstName);
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'Retain this draft until it is saved.');

    // A response to an earlier local edit must not mark a newer edit saved.
    let releaseSave;
    let startedSave;
    const started = new Promise(resolve => { startedSave = resolve; });
    const released = new Promise(resolve => { releaseSave = resolve; });
    await page.route(`**/api/projects/${first}`, async route => {
      if (route.request().method() !== 'PUT') return route.continue();
      startedSave();
      await released;
      await route.fulfill({ response: await route.fetch() });
    });
    await editor().getByLabel('Dialogue').fill('First edit in flight.');
    await Promise.race([
      started,
      new Promise((_, reject) => setTimeout(() => reject(new Error('Autosave did not start')), 10_000)),
    ]);
    await editor().getByLabel('Dialogue').fill('Latest edit while saving.');
    assert.notEqual((await page.getByRole('status').innerText()).trim(), 'Saved');
    releaseSave();
    await waitForData(first, s => s.workspace.panels[0].dialogue === 'Latest edit while saving.');
    await saved();
    await unblockSave(first);

    // History sequence is A -> list -> B. Cancelling must restore the cursor,
    // not replace the list entry with B and silently lose history destinations.
    await projects().click();
    await open(secondName);
    await failSave(second);
    await editor().getByLabel('Dialogue').fill('History navigation draft.');
    await page.getByRole('alert').waitFor();
    for (let attempt = 0; attempt < 2; attempt++) {
      await page.goBack();
      await dialog().getByRole('button', { name: 'Stay', exact: true }).click();
      await page.waitForURL(url => url.searchParams.get('project') === second);
      assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'History navigation draft.');
    }

    // Chromium exposes the supported close/reload warning after user input.
    const warning = page.waitForEvent('dialog', { timeout: 10_000 });
    const reload = page.reload().catch(error => error);
    const unload = await warning;
    assert.equal(unload.type(), 'beforeunload');
    await unload.dismiss();
    await reload;
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'History navigation draft.');
    await page.goBack();
    await dialog().getByRole('button', { name: 'Discard changes and continue', exact: true }).click();
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    await page.waitForURL(url => url.searchParams.get('view') === 'projects');
    assert.equal(new URL(page.url()).searchParams.get('view'), 'projects');
    await unblockSave(second);
    await page.goForward();
    await page.waitForURL(url => url.searchParams.get('project') === second);
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
    assert.equal(new URL(page.url()).searchParams.get('project'), second);
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), '');
    await page.goBack();
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    await page.waitForURL(url => url.searchParams.get('view') === 'projects');
    await page.goBack();
    await page.waitForURL(url => url.searchParams.get('project') === first);
    await page.getByLabel('Project title', { exact: true }).waitFor();
    await saved();
    assert.equal(new URL(page.url()).searchParams.get('project'), first);
    assert.equal(await editor().getByLabel('Dialogue').inputValue(), 'Latest edit while saving.');
    await page.goForward();
    await page.getByRole('heading', { name: 'Your projects', exact: true }).waitFor();
    await page.waitForURL(url => url.searchParams.get('view') === 'projects');

    // The project controls and editor remain usable at phone width.
    await page.setViewportSize({ width: 390, height: 844 });
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone project list overflows');
    fs.mkdirSync('artifacts', { recursive: true });
    await page.screenshot({ path: 'artifacts/projects-phone.png', fullPage: true });
    await open(firstName);
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone project editor overflows');
    await page.screenshot({ path: 'artifacts/project-editor-phone.png', fullPage: true });

    // Unknown identities must never silently open the original project.
    await page.goto(`${baseURL}/?project=${randomUUID()}`);
    await page.getByRole('alert').waitFor();
    assert.equal(await page.getByRole('complementary', { name: 'Panel editor' }).count(), 0);
    await page.goto(`${baseURL}/?project=invalid-project-id`);
    await page.getByRole('alert').waitFor();
    assert.equal(await page.getByRole('complementary', { name: 'Panel editor' }).count(), 0);

    const listResponse = await context.request.get(`${baseURL}/api/projects`);
    assert(listResponse.ok());
    const list = await listResponse.json();
    const projectsBefore = await Promise.all(list.map(summary => read(summary.project_id)));
    const images = [];
    for (const project of projectsBefore) {
      for (const panel of project.workspace.panels) {
        for (const image of panel.images) {
          const response = await context.request.get(`${baseURL}${image.url}`);
          assert(response.ok());
          images.push({ url: image.url, digest: digest(await response.body()) });
        }
      }
    }
    fs.writeFileSync(artifact, JSON.stringify({ list, projects: projectsBefore, images }, null, 2));
    assert.deepEqual(errors, [], 'Browser runtime errors');
    console.log('PASS: create/reopen, metadata autosave, separate project images, unsaved cancel/save/discard, delayed save, history guard, unload warning, phone controls and invalid project recovery');
  } catch (error) {
    fs.mkdirSync('artifacts', { recursive: true });
    console.error('Project browser errors:', errors);
    console.error('Project browser state:', await page.evaluate(() => ({
      url: location.href,
      status: document.querySelector('[role=status]')?.textContent,
      alerts: [...document.querySelectorAll('[role=alert]')].map(node => node.textContent),
      dialog: document.querySelector('#unsaved-dialog')?.outerHTML,
    })).catch(() => 'Page unavailable'));
    await page.screenshot({ path: 'artifacts/project-failure.png', fullPage: true }).catch(() => {});
    throw error;
  } finally {
    await browser.close();
  }
}

(process.argv.includes('--verify-retention') ? verifyRetention() : browserWorkflow())
  .catch(error => { console.error(error); process.exitCode = 1; });
