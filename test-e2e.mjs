import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';
import { tmpdir } from 'node:os';

const root = resolve(new URL('.', import.meta.url).pathname);

function run(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, {
      cwd: root,
      env: { ...process.env, ...options.env },
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (chunk) => {
      stdout += chunk;
    });
    child.stderr.on('data', (chunk) => {
      stderr += chunk;
    });
    child.on('error', reject);
    child.on('close', (code) => {
      if (code === 0) {
        resolvePromise({ stdout, stderr });
      } else {
        reject(new Error(`${command} ${args.join(' ')} failed with ${code}\n${stdout}\n${stderr}`));
      }
    });
  });
}

async function freePort() {
  return new Promise((resolvePromise, reject) => {
    const server = createServer();
    server.listen(0, '127.0.0.1', () => {
      const address = server.address();
      const port = typeof address === 'object' && address ? address.port : 0;
      server.close(() => resolvePromise(port));
    });
    server.on('error', reject);
  });
}

async function waitForJson(url, attempts = 60) {
  let lastError;
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return response.json();
      }
      lastError = new Error(`${response.status} ${await response.text()}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 250));
  }
  throw lastError ?? new Error(`Timed out waiting for ${url}`);
}

async function runAura(workspace, args) {
  const aura = join(root, 'target', 'debug', process.platform === 'win32' ? 'aura.exe' : 'aura');
  return run(aura, ['-C', workspace, ...args]);
}

async function main() {
  console.log('=== Aura end-to-end test ===');

  console.log('[build] compiling Aura CLI and Hub server');
  await run('cargo', ['build', '-p', 'aura-control', '-p', 'aura-server']);

  console.log('[build] compiling Aura Hub web dashboard');
  await run('npm', ['--prefix', 'web', 'run', 'build']);

  const tempRoot = await mkdtemp(join(tmpdir(), 'aura-e2e-'));
  const storageRoot = join(tempRoot, 'hub-storage');
  const ideWorkspace = join(tempRoot, 'ide-workspace');
  const cloneWorkspace = join(tempRoot, 'clone-workspace');
  const port = await freePort();
  const baseUrl = `http://127.0.0.1:${port}`;
  const transportUrl = `${baseUrl}/api/transport/repos/ide-demo`;
  const serverBin = join(root, 'target', 'debug', process.platform === 'win32' ? 'aura-server.exe' : 'aura-server');

  const server = spawn(serverBin, [], {
    cwd: root,
    env: {
      ...process.env,
      AURA_STORAGE_ROOT: storageRoot,
      AURA_API_PORT: String(port),
    },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  server.stdout.on('data', (chunk) => process.stdout.write(`[server] ${chunk}`));
  server.stderr.on('data', (chunk) => process.stderr.write(`[server] ${chunk}`));

  try {
    await waitForJson(`${baseUrl}/api/repos`);

    console.log('[ide] initializing workspace and creating first commit through Aura commands used by IDE');
    await mkdir(join(ideWorkspace, 'src'), { recursive: true });
    await run(join(root, 'target', 'debug', process.platform === 'win32' ? 'aura.exe' : 'aura'), ['init', ideWorkspace]);
    await writeFile(join(ideWorkspace, 'README.md'), '# Aura E2E\n\nCreated from the IDE source control flow.\n');
    await writeFile(join(ideWorkspace, 'src', 'main.rs'), 'fn main() {\n    println!("hello aura");\n}\n');
    await runAura(ideWorkspace, ['add', '-A']);
    await runAura(ideWorkspace, ['commit', '-m', 'IDE initial commit']);

    console.log('[sync] pushing committed workspace to Aura Hub transport endpoint');
    await runAura(ideWorkspace, ['remote', 'add', 'origin', transportUrl]);
    await runAura(ideWorkspace, ['push', 'origin', 'main']);

    console.log('[hub] verifying repository, commits and code browsing APIs');
    const repos = await waitForJson(`${baseUrl}/api/repos`);
    if (!repos.some((repo) => repo.id === 'ide-demo' && repo.storage === 'hosted')) {
      throw new Error(`Expected hosted ide-demo repo, got ${JSON.stringify(repos)}`);
    }

    const log = await waitForJson(`${baseUrl}/api/repo/log?repo=ide-demo`);
    if (!Array.isArray(log) || log[0]?.message !== 'IDE initial commit') {
      throw new Error(`Expected pushed commit in Aura Hub log, got ${JSON.stringify(log)}`);
    }

    const tree = await waitForJson(`${baseUrl}/api/repo/tree?repo=ide-demo`);
    if (!tree.some((entry) => entry.path === 'README.md') || !tree.some((entry) => entry.path === 'src')) {
      throw new Error(`Expected README.md and src in tree, got ${JSON.stringify(tree)}`);
    }

    const file = await waitForJson(`${baseUrl}/api/repo/file?repo=ide-demo&path=src/main.rs`);
    if (!file.content.includes('hello aura')) {
      throw new Error(`Expected source file content from web code API, got ${JSON.stringify(file)}`);
    }

    console.log('[clone] cloning from Aura Hub transport endpoint into another folder');
    await run(join(root, 'target', 'debug', process.platform === 'win32' ? 'aura.exe' : 'aura'), [
      'clone',
      transportUrl,
      cloneWorkspace,
    ]);
    const clonedReadme = await readFile(join(cloneWorkspace, 'README.md'), 'utf8');
    if (!clonedReadme.includes('Aura E2E')) {
      throw new Error('Cloned repository does not contain expected README.md');
    }

    console.log('E2E passed: IDE flow -> commit -> push -> Aura Hub API/web data -> clone.');
  } finally {
    server.kill('SIGTERM');
    await rm(tempRoot, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
