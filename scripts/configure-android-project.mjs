import fs from 'node:fs'
import path from 'node:path'

const root = process.cwd()
const manifestPath = path.join(
  root,
  'src-tauri',
  'gen',
  'android',
  'app',
  'src',
  'main',
  'AndroidManifest.xml',
)
const gradlePath = path.join(root, 'src-tauri', 'gen', 'android', 'app', 'build.gradle.kts')
const buildTaskPath = path.join(
  root,
  'src-tauri',
  'gen',
  'android',
  'buildSrc',
  'src',
  'main',
  'java',
  'com',
  'relayclip',
  'mobile',
  'kotlin',
  'BuildTask.kt',
)

const permissions = [
  'android.permission.INTERNET',
  'android.permission.ACCESS_NETWORK_STATE',
  'android.permission.ACCESS_WIFI_STATE',
  'android.permission.CHANGE_WIFI_MULTICAST_STATE',
  'android.permission.POST_NOTIFICATIONS',
  'android.permission.FOREGROUND_SERVICE',
  'android.permission.FOREGROUND_SERVICE_DATA_SYNC',
]

if (fs.existsSync(manifestPath)) {
  let manifest = fs.readFileSync(manifestPath, 'utf8')

  const missingPermissions = permissions.filter(
    (permission) => !manifest.includes(`android:name="${permission}"`),
  )
  if (missingPermissions.length > 0) {
    const permissionBlock = missingPermissions
      .map((permission) => `    <uses-permission android:name="${permission}" />`)
      .join('\n')
    manifest = manifest.replace(/<manifest([^>]*)>\s*/m, `<manifest$1>\n${permissionBlock}\n`)
  }

  if (!manifest.includes('android:usesCleartextTraffic')) {
    manifest = manifest.replace(
      /<application\b/,
      '<application android:usesCleartextTraffic="true"',
    )
  }

  fs.writeFileSync(manifestPath, manifest)
}

if (fs.existsSync(gradlePath)) {
  let gradle = fs.readFileSync(gradlePath, 'utf8')

  if (!gradle.includes('import java.util.Properties')) {
    gradle = `import java.util.Properties\n${gradle}`
  }

  if (!gradle.includes('val keystorePropertiesFile = rootProject.file("keystore.properties")')) {
    gradle = gradle.replace(
      /android \{/,
      `val keystorePropertiesFile = rootProject.file("keystore.properties")
val keystoreProperties = Properties().apply {
    if (keystorePropertiesFile.exists()) {
        keystorePropertiesFile.inputStream().use(::load)
    }
}

android {
    signingConfigs {
        if (keystorePropertiesFile.exists()) {
            create("release") {
                storeFile = rootProject.file(keystoreProperties["storeFile"] as String)
                storePassword = keystoreProperties["storePassword"] as String
                keyAlias = keystoreProperties["keyAlias"] as String
                keyPassword = keystoreProperties["keyPassword"] as String
            }
        }
    }`,
    )
  }

  if (!gradle.includes('signingConfig = signingConfigs.getByName("release")')) {
    gradle = gradle.replace(
      /getByName\("release"\) \{/,
      `getByName("release") {
            if (keystorePropertiesFile.exists()) {
                signingConfig = signingConfigs.getByName("release")
            }`,
    )
  }

  fs.writeFileSync(gradlePath, gradle)
}

if (fs.existsSync(buildTaskPath)) {
  let buildTask = fs.readFileSync(buildTaskPath, 'utf8')

  if (!buildTask.includes('listOf("--dir", "..", "exec", "tauri", "android", "android-studio-script")')) {
    buildTask = buildTask.replace(
      'val args = listOf("tauri", "android", "android-studio-script");',
      'val args = listOf("--dir", "..", "exec", "tauri", "android", "android-studio-script");',
    )
  }

  fs.writeFileSync(buildTaskPath, buildTask)
}
