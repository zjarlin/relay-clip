import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'

const root = process.cwd()
const timezone = process.env.RELAYCLIP_VERSION_TZ || 'Asia/Shanghai'
const buildNumber = parseBuildNumber(process.env.RELAYCLIP_BUILD_NUMBER)

function parseBuildNumber(rawValue) {
  const parsed = Number.parseInt(rawValue ?? '1', 10)
  if (!Number.isFinite(parsed) || parsed < 1) {
    return 1
  }

  return Math.min(parsed, 2_100_000_000)
}

function getCalendarVersion(timeZone) {
  const formatter = new Intl.DateTimeFormat('en-CA', {
    timeZone,
    year: 'numeric',
    month: 'numeric',
    day: 'numeric',
  })

  const parts = Object.fromEntries(
    formatter
      .formatToParts(new Date())
      .filter((part) => part.type !== 'literal')
      .map((part) => [part.type, part.value]),
  )

  const year = Number(parts.year)
  const month = Number(parts.month)
  const day = Number(parts.day)

  if (!Number.isFinite(year) || !Number.isFinite(month) || !Number.isFinite(day)) {
    throw new Error(`Failed to derive calendar version for timezone ${timeZone}`)
  }

  return `${year}.${month}.${day}`
}

function updateJsonVersion(filePath, version, mutate) {
  if (!fs.existsSync(filePath)) {
    return
  }

  const raw = fs.readFileSync(filePath, 'utf8')
  const json = JSON.parse(raw)
  json.version = version
  mutate?.(json)
  fs.writeFileSync(filePath, `${JSON.stringify(json, null, 2)}\n`)
}

function updateCargoTomlVersion(filePath, version) {
  const raw = fs.readFileSync(filePath, 'utf8')
  const next = raw.replace(/^version = ".*"$/m, `version = "${version}"`)
  fs.writeFileSync(filePath, next)
}

function updateCargoLockVersion(filePath, version) {
  const raw = fs.readFileSync(filePath, 'utf8')
  const next = raw.replace(
    /(\[\[package\]\]\r?\nname = "relayclip"\r?\nversion = ")([^"]+)(")/m,
    `$1${version}$3`,
  )
  fs.writeFileSync(filePath, next)
}

const version = getCalendarVersion(timezone)

updateJsonVersion(path.join(root, 'package.json'), version)
updateJsonVersion(path.join(root, 'src-tauri', 'tauri.conf.json'), version)
updateJsonVersion(path.join(root, 'src-tauri', 'tauri.android.conf.json'), version, (json) => {
  json.bundle ??= {}
  json.bundle.android ??= {}
  json.bundle.android.versionCode = buildNumber
})
updateJsonVersion(path.join(root, 'src-tauri', 'tauri.ios.conf.json'), version, (json) => {
  json.bundle ??= {}
  json.bundle.iOS ??= {}
  json.bundle.iOS.bundleVersion = String(buildNumber)
})
updateCargoTomlVersion(path.join(root, 'src-tauri', 'Cargo.toml'), version)
updateCargoLockVersion(path.join(root, 'src-tauri', 'Cargo.lock'), version)

console.log(JSON.stringify({ version, buildNumber }))
