import { buildAndroidDebugApk } from './android-build-utils.mjs'

const built = buildAndroidDebugApk()

console.log(`Android debug APK ready: ${built.apkPath}`)
