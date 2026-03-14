import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = path.dirname(__filename)
const repoRoot = path.resolve(__dirname, '..')
const srcTauriDir = path.join(repoRoot, 'src-tauri')
const androidProjectDir = path.join(srcTauriDir, 'gen', 'android')
const androidAppDir = path.join(androidProjectDir, 'app')

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

function deepMerge(base, override) {
  if (!isObject(base) || !isObject(override)) {
    return structuredClone(override)
  }

  const merged = { ...base }
  for (const [key, value] of Object.entries(override)) {
    if (isObject(value) && isObject(merged[key])) {
      merged[key] = deepMerge(merged[key], value)
    } else {
      merged[key] = structuredClone(value)
    }
  }
  return merged
}

function readJson(jsonPath) {
  return JSON.parse(fs.readFileSync(jsonPath, 'utf8'))
}

function writeIfChanged(filePath, contents) {
  if (fs.existsSync(filePath) && fs.readFileSync(filePath, 'utf8') === contents) {
    return
  }

  fs.mkdirSync(path.dirname(filePath), { recursive: true })
  fs.writeFileSync(filePath, contents)
}

function ensureExists(targetPath, label) {
  if (!fs.existsSync(targetPath)) {
    throw new Error(`${label} not found: ${targetPath}`)
  }
  return targetPath
}

function findLatestNdk(androidHome) {
  const ndkRoot = path.join(androidHome, 'ndk')
  if (!fs.existsSync(ndkRoot)) {
    return null
  }

  const candidates = fs
    .readdirSync(ndkRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((left, right) => right.localeCompare(left, undefined, { numeric: true }))

  return candidates[0] ? path.join(ndkRoot, candidates[0]) : null
}

function detectJavaHome() {
  const candidates = [
    process.env.JAVA_HOME,
    'C:\\Program Files\\Microsoft\\jdk-17.0.18.8-hotspot',
    'C:\\Program Files\\Android\\Android Studio\\jbr',
  ].filter(Boolean)

  const match = candidates.find((candidate) => fs.existsSync(candidate))
  if (!match) {
    throw new Error('JAVA_HOME is not configured and no supported local JDK was found.')
  }
  return match
}

function detectAndroidHome() {
  const candidates = [
    process.env.ANDROID_HOME,
    process.env.ANDROID_SDK_ROOT,
    path.join(os.homedir(), 'AppData', 'Local', 'Android', 'Sdk'),
  ].filter(Boolean)

  const match = candidates.find((candidate) => fs.existsSync(candidate))
  if (!match) {
    throw new Error('ANDROID_HOME / ANDROID_SDK_ROOT is not configured and no Android SDK was found.')
  }
  return match
}

function detectNdkHome(androidHome) {
  const candidates = [process.env.NDK_HOME, findLatestNdk(androidHome)].filter(Boolean)
  const match = candidates.find((candidate) => fs.existsSync(candidate))
  if (!match) {
    throw new Error(`NDK_HOME is not configured and no Android NDK was found under ${androidHome}.`)
  }
  return match
}

function parseLibName() {
  const cargoTomlPath = path.join(srcTauriDir, 'Cargo.toml')
  const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8')
  const libSection = cargoToml.match(/\[lib\][\s\S]*?name\s*=\s*"([^"]+)"/)
  if (libSection?.[1]) {
    return libSection[1]
  }

  const packageSection = cargoToml.match(/\[package\][\s\S]*?name\s*=\s*"([^"]+)"/)
  if (packageSection?.[1]) {
    return packageSection[1].replace(/-/g, '_')
  }

  throw new Error(`Unable to determine Rust library name from ${cargoTomlPath}.`)
}

function parseMergedConfig() {
  const base = readJson(path.join(srcTauriDir, 'tauri.conf.json'))
  const android = readJson(path.join(srcTauriDir, 'tauri.android.conf.json'))
  return deepMerge(base, android)
}

function computeVersionCode(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)$/.exec(version ?? '')
  if (!match) {
    return 1
  }

  const [, majorRaw, minorRaw, patchRaw] = match
  const major = Number(majorRaw)
  const minor = Number(minorRaw)
  const patch = Number(patchRaw)
  const versionCode = major * 1_000_000 + minor * 1_000 + patch

  if (versionCode < 1 || versionCode > 2_100_000_000) {
    throw new Error(`Derived Android versionCode ${versionCode} is invalid for version ${version}.`)
  }

  return versionCode
}

function escapeLocalProperty(value) {
  return value.replace(/\\/g, '\\\\').replace(/:/g, '\\:')
}

function commandName(base) {
  return process.platform === 'win32' ? `${base}.cmd` : base
}

function quoteForCmd(value) {
  if (!/[ \t"&()<>^|]/.test(value)) {
    return value
  }
  return `"${value.replace(/"/g, '\\"')}"`
}

function runCommand(command, args, options = {}) {
  const useShell = process.platform === 'win32' && /\.(cmd|bat)$/i.test(command)
  const result = spawnSync(
    useShell ? process.env.ComSpec ?? 'cmd.exe' : command,
    useShell ? ['/d', '/s', '/c', [command, ...args].map(quoteForCmd).join(' ')] : args,
    {
    cwd: options.cwd ?? repoRoot,
    env: options.env ?? process.env,
    stdio: 'inherit',
    shell: false,
    windowsHide: false,
  })

  if (result.status !== 0) {
    throw new Error(`Command failed: ${command} ${args.join(' ')}`)
  }
}

function buildEnvironment(config) {
  const javaHome = detectJavaHome()
  const androidHome = detectAndroidHome()
  const ndkHome = detectNdkHome(androidHome)
  const minSdk = config.bundle?.android?.minSdkVersion ?? 26
  const identifier = String(config.identifier).replace(/-/g, '_')
  const library = parseLibName()
  const toolchainBin = ensureExists(
    path.join(ndkHome, 'toolchains', 'llvm', 'prebuilt', 'windows-x86_64', 'bin'),
    'Android NDK LLVM toolchain',
  )
  const generatedKotlinDir = path.join(
    androidAppDir,
    'src',
    'main',
    'java',
    ...identifier.split('.'),
    'generated',
  )
  const linker = ensureExists(
    path.join(toolchainBin, `aarch64-linux-android${minSdk}-clang.cmd`),
    'Android target linker',
  )
  const cxx = ensureExists(
    path.join(toolchainBin, `aarch64-linux-android${minSdk}-clang++.cmd`),
    'Android target C++ compiler',
  )
  const ar = ensureExists(path.join(toolchainBin, 'llvm-ar.exe'), 'Android llvm-ar')

  fs.mkdirSync(generatedKotlinDir, { recursive: true })

  const env = {
    ...process.env,
    JAVA_HOME: javaHome,
    ANDROID_HOME: androidHome,
    ANDROID_SDK_ROOT: androidHome,
    NDK_HOME: ndkHome,
    PATH: `${toolchainBin}${path.delimiter}${process.env.PATH ?? ''}`,
    TAURI_ANDROID_PROJECT_PATH: androidProjectDir,
    TAURI_ANDROID_PACKAGE_UNESCAPED: identifier,
    WRY_ANDROID_PACKAGE: identifier,
    WRY_ANDROID_LIBRARY: library,
    WRY_ANDROID_KOTLIN_FILES_OUT_DIR: generatedKotlinDir,
    CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: linker,
    CARGO_TARGET_AARCH64_LINUX_ANDROID_AR: ar,
    CC_aarch64_linux_android: linker,
    CXX_aarch64_linux_android: cxx,
    AR_aarch64_linux_android: ar,
  }

  return {
    env,
    javaHome,
    androidHome,
    ndkHome,
    toolchainBin,
    generatedKotlinDir,
    identifier,
    library,
    minSdk,
  }
}

function writeAndroidSupportFiles(config, envInfo, profile) {
  const version = config.version ?? '1.0.0'
  const versionCode = config.bundle?.android?.versionCode ?? computeVersionCode(version)
  const tauriProperties = [
    '// THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.',
    `tauri.android.versionName=${version}`,
    `tauri.android.versionCode=${versionCode}`,
    '',
  ].join('\n')

  writeIfChanged(path.join(androidAppDir, 'tauri.properties'), tauriProperties)
  writeIfChanged(
    path.join(androidProjectDir, 'local.properties'),
    [
      `sdk.dir=${escapeLocalProperty(envInfo.androidHome)}`,
      `ndk.dir=${escapeLocalProperty(envInfo.ndkHome)}`,
      '',
    ].join('\n'),
  )

  const tauriConfigPath = path.join(androidAppDir, 'src', 'main', 'assets', 'tauri.conf.json')
  writeIfChanged(tauriConfigPath, `${JSON.stringify(config, null, 2)}\n`)

  const abiDir = path.join(androidAppDir, 'src', 'main', 'jniLibs', 'arm64-v8a')
  fs.mkdirSync(abiDir, { recursive: true })

  const profileDir = profile === 'release' ? 'release' : 'debug'
  return {
    abiDir,
    profileDir,
    soPath: path.join(srcTauriDir, 'target', 'aarch64-linux-android', profileDir, `lib${envInfo.library}.so`),
  }
}

function copyNativeLibrary(soPath, abiDir) {
  ensureExists(soPath, 'Built Android shared library')
  const destination = path.join(abiDir, path.basename(soPath))
  fs.copyFileSync(soPath, destination)
  return destination
}

export function prepareAndroidBuild({ profile = 'debug', buildFrontend = false } = {}) {
  ensureExists(path.join(androidProjectDir, 'gradlew.bat'), 'Generated Android project')

  const config = parseMergedConfig()
  const envInfo = buildEnvironment(config)
  const supportFiles = writeAndroidSupportFiles(config, envInfo, profile)

  if (buildFrontend) {
    runCommand(commandName('pnpm'), ['build'], { cwd: repoRoot, env: envInfo.env })
  }

  const cargoArgs = ['build', '--manifest-path', path.join(srcTauriDir, 'Cargo.toml'), '--target', 'aarch64-linux-android']
  if (profile === 'release') {
    cargoArgs.push('--release')
  }
  runCommand('cargo', cargoArgs, { cwd: repoRoot, env: envInfo.env })

  const copiedSoPath = copyNativeLibrary(supportFiles.soPath, supportFiles.abiDir)

  return {
    config,
    env: envInfo.env,
    envInfo,
    androidProjectDir,
    gradlewPath: path.join(androidProjectDir, 'gradlew.bat'),
    copiedSoPath,
    apkPath: path.join(
      androidAppDir,
      'build',
      'outputs',
      'apk',
      'arm64',
      profile,
      `app-arm64-${profile}.apk`,
    ),
  }
}

export function buildAndroidDebugApk() {
  const prepared = prepareAndroidBuild({ profile: 'debug', buildFrontend: true })
  runCommand(
    prepared.gradlewPath,
    ['assembleArm64Debug', '-x', 'rustBuildArm64Debug'],
    { cwd: prepared.androidProjectDir, env: prepared.env },
  )
  ensureExists(prepared.apkPath, 'Android debug APK')
  return prepared
}
