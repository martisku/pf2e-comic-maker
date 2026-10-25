// Disposable services only. Browser backup transport and complete milestone graph.
const { chromium } = require('playwright');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { randomUUID, createHash } = require('node:crypto');
const baseURL = process.env.COMIC_TEST_URL || 'http://127.0.0.1:8080';
const digest = bytes => createHash('sha256').update(bytes).digest('hex');
const artifact = 'artifacts/backup-retention.json';
(async () => {
  const browser = await chromium.launch({ executablePath: process.env.COMIC_CHROMIUM === 'bundled' ? undefined : (process.env.COMIC_CHROMIUM || '/usr/bin/chromium'), headless: true, args: ['--no-sandbox'] });
  const context = await browser.newContext({ viewport: { width: 1440, height: 1000 }, httpCredentials: { username: 'comic', password: process.env.COMIC_TEST_PASSWORD || '', send: 'always' }, extraHTTPHeaders: { Origin: new URL(baseURL).origin } });
  const page = await context.newPage();
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  const api = async (path, method = 'get', options = {}) => {
    const response = await context.request[method](`${baseURL}${path}`, options);
    assert(response.ok(), `${method} ${path}: ${response.status()} ${await response.text()}`);
    return response;
  };
  const read = async id => (await api(`/api/projects/${id}`)).json();
  const save = async snapshot => (await api(`/api/projects/${snapshot.project_id}`, 'put', { data: snapshot })).json();
  const button = name => page.getByRole('button', { name, exact: true });
  const saved = () => page.waitForFunction(() => document.querySelector('[role=status]')?.textContent.trim() === 'Saved');
  const originals = async snapshot => {
    const images = [...snapshot.workspace.panels.flatMap(p => p.images), ...snapshot.workspace.assets.flatMap(a => a.reference_images)];
    return Promise.all(images.map(async image => ({ url: image.url, digest: digest(await (await api(image.url)).body()) })));
  };
  const normalized = snapshot => {
    const copy = structuredClone(snapshot); delete copy.project_id; delete copy.revision;
    const w = copy.workspace;
    const assets = new Map(w.assets.map((a, i) => [a.id, `asset-${i}`]));
    for (const asset of w.assets) { asset.id = assets.get(asset.id); asset.reference_images.forEach((r, i) => { r.id = `ref-${i}`; r.url = `ref-${i}`; }); }
    for (const panel of w.panels) {
      const images = new Map(panel.images.map((image, i) => [image.id, `image-${i}`]));
      panel.selected_image = images.get(panel.selected_image) || null;
      panel.images.forEach((image, i) => { image.id = `image-${i}`; image.url = `image-${i}`; });
      panel.assets.characters = panel.assets.characters.map(id => assets.get(id));
      panel.assets.equipment = panel.assets.equipment.map(id => assets.get(id));
      panel.assets.location = assets.get(panel.assets.location) || null;
    }
    return copy;
  };
  try {
    fs.mkdirSync('artifacts', { recursive: true });
    if (process.argv.includes('--verify-retention')) {
      const before = JSON.parse(fs.readFileSync(artifact));
      for (const snapshot of before.projects) assert.deepEqual(await read(snapshot.project_id), snapshot);
      for (const image of before.images) assert.equal(digest(await (await api(image.url)).body()), image.digest);
      console.log('PASS: fresh runtime retains original and restored complete projects and every alternative/reference original'); return;
    }
    await page.goto(`${baseURL}/?view=projects`);
    const png = Buffer.from(await page.evaluate(() => { const c = document.createElement('canvas'); c.width = 24; c.height = 18; c.getContext('2d').fillRect(0, 0, 24, 18); return c.toDataURL('image/png').split(',')[1]; }), 'base64');
    let source = await (await api('/api/projects', 'post', { data: { project_id: randomUUID(), name: `Backup milestone ${randomUUID().slice(0, 8)}`, description: 'Complete project description' } })).json();
    const w = source.workspace;
    w.panels = w.panels.slice(0, 3);
    w.assets = ['Character', 'Location', 'Equipment'].map(kind => ({ id: randomUUID(), kind, name: `${kind} shared`, description: `${kind} description`, design_notes: `${kind} design`, character_size: kind === 'Character' ? 'Small' : null, reference_images: [] }));
    w.panels.forEach((panel, i) => Object.assign(panel, { title: `Ordered panel ${i + 1}`, action: `Action ${i}`, dialogue: `Dialogue ${i}`, expression: `Expression ${i}`, framing: `Framing ${i}`, mood: `Mood ${i}`, production_notes: `Production ${i}`, fill: i !== 1, crop_x: 17 + i, crop_y: 78 - i, assets: { characters: [w.assets[0].id], location: w.assets[1].id, equipment: [w.assets[2].id] } }));
    w.chapters = [
      { id: 11, title: 'Opening', description: 'Opening description', panel_ids: [3, 1], pages: [{ id: 8, layout: 'Wide', slots: [1, null, 3] }, { id: 4, layout: 'Full', slots: [null] }] },
      { id: 23, title: 'Closing', description: 'Closing description', panel_ids: [2], pages: [{ id: 19, layout: 'Grid', slots: [null, 2, null, null] }] },
    ];
    source = await save(source);
    for (const panel of source.workspace.panels) {
      for (let i = 0; i < 2; i++) source = await (await api(`/api/projects/${source.project_id}/panels/${panel.id}/images`, 'post', { headers: { 'Content-Type': 'image/png', 'x-comic-revision': String(source.revision), 'x-comic-upload-id': randomUUID() }, data: png })).json();
    }
    for (const asset of source.workspace.assets) source = await (await api(`/api/projects/${source.project_id}/assets/${asset.id}/references`, 'post', { headers: { 'Content-Type': 'image/png', 'x-comic-revision': String(source.revision), 'x-comic-upload-id': randomUUID() }, data: png })).json();
    source.workspace.panels[0].selected_image = source.workspace.panels[0].images[0].id;
    source.workspace.assets.forEach(asset => Object.assign(asset.reference_images[0], { label: `${asset.kind} original`, description: 'Original visual reference' }));
    source = await save(source);
    await page.goto(`${baseURL}/?project=${source.project_id}`); await saved();
    const downloadEvent = page.waitForEvent('download'); await button('Export project backup').click();
    const download = await downloadEvent; assert(download.suggestedFilename().endsWith('.comicbackup'));
    const archivePath = 'artifacts/milestone.comicbackup'; await download.saveAs(archivePath);
    const archive = fs.readFileSync(archivePath); assert(archive.length > png.length * 9);
    // Advance source after export: restoring this older backup must preserve both versions.
    const older = structuredClone(source); source.workspace.panels[0].action = 'Source advanced after backup'; source = await save(source);
    const restore = async bytes => {
      await page.goto(`${baseURL}/?view=projects`);
      await page.getByLabel('Restore project backup', { exact: true }).setInputFiles({ name: 'milestone.comicbackup', mimeType: 'application/octet-stream', buffer: bytes });
      await button('Restore backup').click(); await page.getByLabel('Project title', { exact: true }).waitFor(); await saved();
      return read(new URL(page.url()).searchParams.get('project'));
    };
    let copy = await restore(archive);
    assert.notEqual(copy.project_id, source.project_id); assert.deepEqual(normalized(copy), normalized(older));
    const oldIDs = new Set([...older.workspace.assets.map(a => a.id), ...older.workspace.assets.flatMap(a => a.reference_images.map(r => r.id)), ...older.workspace.panels.flatMap(p => p.images.map(i => i.id))]);
    for (const id of [...copy.workspace.assets.map(a => a.id), ...copy.workspace.assets.flatMap(a => a.reference_images.map(r => r.id)), ...copy.workspace.panels.flatMap(p => p.images.map(i => i.id))]) assert(!oldIDs.has(id), `Identity reused ${id}`);
    assert.deepEqual((await originals(copy)).map(i => i.digest), (await originals(older)).map(i => i.digest));
    await page.getByLabel('Action', { exact: true }).fill('Copy edited after restoration'); await saved();
    await page.reload(); await saved(); assert.equal(await page.getByLabel('Action', { exact: true }).inputValue(), 'Copy edited after restoration');
    copy = await read(copy.project_id); assert.deepEqual(await read(source.project_id), source);
    await page.getByRole('navigation', { name: 'Workspace views' }).getByRole('button', { name: 'Asset library' }).click();
    await button('Character shared').click(); await button('Open reference Character original').click();
    const lightbox = page.getByRole('dialog', { name: 'Reference image: Character original' }); await lightbox.getByRole('img').evaluate(image => image.decode()); await button('Close reference image').click();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`${baseURL}/?view=projects`);
    assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), 'Phone backup controls overflow');
    await page.screenshot({ path: 'artifacts/backup-phone.png', fullPage: true });
    const beforeInvalid = await (await api('/api/projects')).json();
    const corrupted = Buffer.from(archive); corrupted[corrupted.length - 1] ^= 255;
    const unsupported = Buffer.from(archive); unsupported[7] = 255;
    for (const bytes of [Buffer.from('not a backup'), unsupported, corrupted, archive.subarray(0, archive.length - 1)]) {
      await page.goto(`${baseURL}/?view=projects`);
      await page.getByLabel('Restore project backup', { exact: true }).setInputFiles({ name: 'invalid.comicbackup', mimeType: 'application/octet-stream', buffer: bytes });
      await button('Restore backup').click(); await page.getByRole('alert').waitFor();
      assert.deepEqual(await (await api('/api/projects')).json(), beforeInvalid);
      assert.deepEqual(await read(source.project_id), source); assert.deepEqual(await read(copy.project_id), copy);
    }
    fs.writeFileSync(artifact, JSON.stringify({ projects: [source, copy], images: [...await originals(source), ...await originals(copy)] }, null, 2));
    assert.deepEqual(errors, [], 'Browser runtime errors');
    console.log('PASS: browser complete milestone backup, original bytes, shared relationships, isolated old copy, edit/reopen, invalid archives and phone controls');
  } catch (error) {
    fs.mkdirSync('artifacts', { recursive: true }); console.error('Backup browser runtime errors:', errors); console.error(await page.locator('body').innerText().catch(() => 'Unavailable'));
    await page.screenshot({ path: 'artifacts/backup-failure.png', fullPage: true }).catch(() => {}); throw error;
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
