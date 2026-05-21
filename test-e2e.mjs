import { spawn } from 'child_process';
import { mkdtemp, writeFile } from 'fs/promises';
import { join } from 'path';
import { tmpdir } from 'os';
import { rm } from 'fs/promises';

async function run() {
    console.log("=== Aura VCS E2E Tests ===");
    
    // Create a temporary directory for our test repository
    const repoPath = await mkdtemp(join(tmpdir(), 'aura-test-'));
    console.log(`[INFO] Created temporary test repository at: ${repoPath}`);

    // Build the server first to ensure it's ready
    console.log("[INFO] Building Rust server...");
    await new Promise((resolve, reject) => {
        const build = spawn('cargo', ['build', '--release', '--manifest-path', 'server/Cargo.toml']);
        build.on('close', code => code === 0 ? resolve() : reject(new Error('Build failed')));
    });

    console.log("[INFO] Starting server...");
    const server = spawn('server/target/release/aura-server', [], {
        env: { ...process.env, AURA_REPO_PATH: repoPath }
    });

    server.stdout.on('data', data => console.log(`[Server] ${data.toString().trim()}`));
    server.stderr.on('data', data => console.error(`[Server ERR] ${data.toString().trim()}`));

    // Wait for the server to spin up
    await new Promise(resolve => setTimeout(resolve, 3000));

    try {
        console.log("\n--- Test 1: Check initial status ---");
        let res = await fetch('http://localhost:3000/api/repo/status');
        let data = await res.json();
        console.log('Response:', data);
        if (data.branch !== 'main') throw new Error("Expected branch 'main'");

        console.log("\n--- Test 2: Create a file and check status ---");
        await writeFile(join(repoPath, 'hello.txt'), 'Hello Aura VCS!');
        res = await fetch('http://localhost:3000/api/repo/status');
        data = await res.json();
        console.log('Response:', data);
        if (!data.untracked.includes('hello.txt')) throw new Error("File should be untracked");

        console.log("\n--- Test 3: Commit changes (auto-adds files) ---");
        res = await fetch('http://localhost:3000/api/repo/commit', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ message: 'Initial test commit' })
        });
        data = await res.json();
        console.log('Response:', data);
        if (!data.hash) throw new Error("No commit hash returned");
        const commitHash = data.hash;

        console.log("\n--- Test 4: Verify commit log ---");
        res = await fetch('http://localhost:3000/api/repo/log');
        data = await res.json();
        console.log('Response:', data);
        if (data.length === 0 || data[0].message !== 'Initial test commit') {
            throw new Error("Commit not found in log");
        }

        console.log("\n✅ ALL TESTS PASSED SUCCESSFULLY!");
    } catch (e) {
        console.error("\n❌ TEST FAILED:", e.message);
        process.exitCode = 1;
    } finally {
        console.log("\n[INFO] Cleaning up...");
        server.kill();
        await rm(repoPath, { recursive: true, force: true });
    }
}

run();