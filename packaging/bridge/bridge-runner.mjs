#!/usr/bin/env node
import {spawn} from 'node:child_process'
import {existsSync} from 'node:fs'
import {dirname, join} from 'node:path'
import {fileURLToPath} from 'node:url'

const [commandId, ...args] = process.argv.slice(2)

if (!commandId) {
  console.error('Usage: bridge-runner <command-id> [...args]')
  process.exit(2)
}

const bridgeDir = dirname(fileURLToPath(import.meta.url))
const runnerCandidates = [
  join(bridgeDir, 'node-cli', 'bin', 'run.js'),
  join(bridgeDir, 'node-cli', 'packages', 'cli', 'bin', 'run.js'),
]
const cliRunner = runnerCandidates.find((candidate) => existsSync(candidate))
const commandParts = commandId.split(':').filter(Boolean)
const [major = 0, minor = 0] = process.versions.node.split('.').map(Number)

if (major < 22 || (major === 22 && minor < 12)) {
  console.error(`Shopify CLI bridge requires Node.js 22.12.0 or newer; found ${process.versions.node}. Reinstall the release artifact.`)
  process.exit(1)
}

if (!cliRunner) {
  console.error('Shopify CLI bridge payload is missing bin/run.js')
  process.exit(1)
}

const child = spawn(process.execPath, [cliRunner, ...commandParts, ...args], {
  cwd: process.cwd(),
  env: process.env,
  stdio: 'inherit',
})

child.on('error', (error) => {
  console.error(`Failed to start Shopify CLI bridge: ${error.message}`)
  process.exit(1)
})

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
    return
  }
  process.exit(code ?? 1)
})
