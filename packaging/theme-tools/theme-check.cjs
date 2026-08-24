#!/usr/bin/env node
'use strict'

const fs = require('node:fs')
const path = require('node:path')
const YAML = require('yaml')
const {
  LegacyIdentifiers,
  Severity,
  applyFixToString,
  autofix,
  loadConfig,
  path: themePath,
  themeCheckRun,
} = require('@shopify/theme-check-node')

function parseArgs(argv) {
  const options = {root: process.cwd(), failLevel: 'error', output: 'text'}
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === '--auto-correct') options.autoCorrect = true
    else if (arg === '--init') options.init = true
    else if (arg === '--list') options.list = true
    else if (arg === '--print') options.print = true
    else if (arg === '--config') options.config = argv[++index]
    else if (arg === '--environment') options.environment = argv[++index]
    else if (arg === '--fail-level') options.failLevel = argv[++index]
    else if (arg === '--output') options.output = argv[++index]
    else if (!arg.startsWith('-')) options.root = path.resolve(arg)
  }
  if (options.config?.startsWith(':')) {
    options.config = LegacyIdentifiers.get(options.config.slice(1)) ?? options.config
  }
  return options
}

function severityLabel(severity) {
  if (severity === Severity.ERROR) return 'error'
  if (severity === Severity.WARNING) return 'warning'
  return 'info'
}

function groupedOffenses(offenses) {
  const grouped = new Map()
  for (const offense of offenses) {
    const file = themePath.fsPath(offense.uri)
    if (!grouped.has(file)) grouped.set(file, [])
    grouped.get(file).push(offense)
  }
  for (const values of grouped.values()) values.sort((left, right) => left.severity - right.severity)
  return [...grouped.entries()].sort(([left], [right]) => left.localeCompare(right))
}

function counts(offenses) {
  return offenses.reduce((result, offense) => {
    const label = severityLabel(offense.severity)
    result[label] = (result[label] ?? 0) + 1
    return result
  }, {})
}

function renderJson(offenses, environment) {
  return groupedOffenses(offenses).map(([file, values]) => {
    const total = counts(values)
    return {
      environment,
      path: file,
      offenses: values.map((offense) => ({
        check: offense.check,
        severity: severityLabel(offense.severity),
        start_row: offense.start.line,
        start_column: offense.start.character,
        end_row: offense.end.line,
        end_column: offense.end.character,
        message: offense.message,
      })),
      errorCount: total.error ?? 0,
      warningCount: total.warning ?? 0,
      infoCount: total.info ?? 0,
    }
  })
}

function renderText(offenses, root, environment) {
  for (const [file, values] of groupedOffenses(offenses)) {
    const relative = path.relative(root, file)
    process.stdout.write(`${environment ? `[${environment}] ` : ''}${relative}\n`)
    const lines = fs.readFileSync(file, 'utf8').split('\n')
    for (const offense of values) {
      const snippet = lines.slice(offense.start.line, offense.end.line + 1)
        .map((line, index) => `${offense.start.line + index + 1}  ${snippetLength(offense, line)}`)
        .join('\n')
      process.stdout.write(`[${severityLabel(offense.severity)}]: ${offense.check}\n${offense.message}\n\n${snippet}\n\n`)
    }
  }
}

function snippetLength(offense, line) {
  return offense.start.line === offense.end.line ? line.trim() : line
}

function threshold(level) {
  if (level === 'crash') return undefined
  if (level === 'error') return Severity.ERROR
  if (level === 'warning' || level === 'suggestion') return Severity.WARNING
  return Severity.INFO
}

async function initialize(root) {
  const target = path.join(root, '.theme-check.yml')
  if (fs.existsSync(target)) {
    process.stdout.write(`.theme-check.yml already exists at ${root}\n`)
    return
  }
  const {settings} = await loadConfig(undefined, root)
  const checks = YAML.stringify(settings).split('\n').map((line) => `# ${line}`).join('\n')
  fs.writeFileSync(target, `${YAML.stringify({extends: 'theme-check:recommended', ignore: ['node_modules/**']})}${checks}`)
  process.stdout.write(`Created .theme-check.yml at ${root}\n`)
}

async function printConfig(options, list) {
  const {ignore, settings, rootUri, checks} = await loadConfig(options.config, options.root)
  let value
  if (list) {
    const patterns = [...new Set(ignore ?? [])]
    value = Object.fromEntries(Object.entries(settings).flatMap(([code, setting]) => {
      if (!setting.enabled) return []
      const {severity, enabled, ...additional} = setting
      const check = checks.find((candidate) => candidate.meta.code === code)
      return [[code, {
        severity: severityLabel(severity ?? Severity.INFO),
        ...(check?.meta?.docs ? {description: check.meta.docs.description, doc: check.meta.docs.url} : {}),
        ignored_patterns: `[${patterns.join(', ')}]`,
        ...additional,
      }]]
    }))
  } else {
    value = {extends: [], ignore: [...new Set(ignore ?? [])], rootUri, ...settings}
  }
  if (options.environment) value = {[options.environment]: value}
  process.stdout.write(YAML.stringify(value))
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  if (options.init) return initialize(options.root)
  if (options.print || options.list) return printConfig(options, options.list)

  const {theme, offenses} = await themeCheckRun(options.root, options.config, console.error.bind(console))
  if (options.autoCorrect) {
    await autofix(theme, offenses, async (sourceCode, fix) => {
      fs.writeFileSync(themePath.fsPath(sourceCode.uri), applyFixToString(sourceCode.source, fix))
    })
  }
  if (options.output === 'json') {
    process.stdout.write(`${JSON.stringify(renderJson(offenses, options.environment), null, 2)}\n`)
  } else {
    renderText(offenses, options.root, options.environment)
    const total = counts(offenses)
    const files = groupedOffenses(offenses).length
    process.stdout.write(`${theme.length} files inspected`)
    if (offenses.length === 0) process.stdout.write(' with no offenses found.\n')
    else {
      process.stdout.write(` with ${offenses.length} total offenses found across ${files} files.`)
      if (total.error) process.stdout.write(`\n${total.error} errors.`)
      if (total.warning) process.stdout.write(`\n${total.warning} warnings.`)
      if (total.info) process.stdout.write(`\n${total.info} info issues.`)
      process.stdout.write('\n')
    }
  }
  const failAt = threshold(options.failLevel)
  process.exitCode = failAt !== undefined && offenses.some((offense) => offense.severity <= failAt) ? 1 : 0
}

main().catch((error) => {
  console.error(error?.stack ?? String(error))
  process.exitCode = 1
})
