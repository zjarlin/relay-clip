# RelayClip Mobile Build Status

更新时间: 2026-03-14

## 当前结论

- Android 方向已经补上可重复执行的本机构建链路，目标是先稳定产出 `arm64 debug APK`。
- iOS 仍然保持脚手架状态，当前这台 Windows 机器不承担 iOS 打包。
- Android 不再依赖 Windows Developer Mode 的符号链接能力，而是改为:
  - 先用 Rust 交叉编译产出 `.so`
  - 再复制到 `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a`
  - 最后让 Gradle 跳过 `rustBuildArm64Debug`，直接组装 APK

## 这次已经落到仓库里的内容

- 新增 Android 构建辅助脚本:
  - `scripts/android-build-utils.mjs`
  - `scripts/prepare-android-build.mjs`
  - `scripts/build-android-debug.mjs`
- 新增 package scripts:
  - `pnpm android:prepare`
  - `pnpm android:debug:apk`
- 构建脚本会自动完成这些动作:
  - 探测 `JAVA_HOME` / `ANDROID_HOME` / `ANDROID_SDK_ROOT` / `NDK_HOME`
  - 合并 `src-tauri/tauri.conf.json` 与 `src-tauri/tauri.android.conf.json`
  - 写入 `src-tauri/gen/android/app/src/main/assets/tauri.conf.json`
  - 生成 `src-tauri/gen/android/app/tauri.properties`
  - 写入 `src-tauri/gen/android/local.properties`
  - 为 Android 目标编译 Rust 动态库
  - 触发 Tauri / Wry 的 Android codegen，自动生成:
    - `src-tauri/gen/android/tauri.settings.gradle`
    - `src-tauri/gen/android/app/tauri.build.gradle.kts`
    - `src-tauri/gen/android/app/src/main/java/.../generated/*.kt`
  - 复制 `librelayclip_lib.so` 到 `jniLibs/arm64-v8a`
  - 运行 `gradlew.bat assembleArm64Debug -x rustBuildArm64Debug`

## 当前机器已确认存在的工具链

- Java:
  - `C:\Program Files\Microsoft\jdk-17.0.18.8-hotspot`
- Android SDK:
  - `C:\Users\zjarl\AppData\Local\Android\Sdk`
- Android NDK:
  - `C:\Users\zjarl\AppData\Local\Android\Sdk\ndk\27.0.12077973`
- Rust target:
  - `aarch64-linux-android`

## 当前已验证通过的关键步骤

- `cargo build --target aarch64-linux-android` 在补齐 Android clang / ar / env 后可通过
- Tauri build script 已能自动生成:
  - `src-tauri/gen/android/tauri.settings.gradle`
  - `src-tauri/gen/android/app/tauri.build.gradle.kts`
  - `src-tauri/gen/android/app/src/main/java/com/relayclip/mobile/generated/*`
- Rust 产物已能生成到:
  - `src-tauri/target/aarch64-linux-android/debug/librelayclip_lib.so`

## 推荐命令

首次或仅做准备:

```powershell
pnpm android:prepare
```

直接生成 Android debug APK:

```powershell
pnpm android:debug:apk
```

## 预期 APK 输出位置

```text
src-tauri/gen/android/app/build/outputs/apk/arm64/debug/app-arm64-debug.apk
```

绝对路径:

```text
C:\Users\zjarl\Documents\Playground\src-tauri\gen\android\app\build\outputs\apk\arm64\debug\app-arm64-debug.apk
```

## 剩余边界

- 当前脚本只覆盖 Android `arm64 debug APK`
- 还没有做:
  - Android release 签名包
  - iOS 本机构建
  - GitHub Actions 的移动端正式打包流水线
- 当前 APK 更适合 smoke test:
  - 安装是否成功
  - UI 是否能启动
  - 移动端脚手架桥接是否能跑通
