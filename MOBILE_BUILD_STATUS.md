# RelayClip Mobile Build Status

更新日期: 2026-03-13

## 这次已经完成的事情

### 仓库内改动

- 已为项目补上 Android / iOS 的 Tauri Mobile 基础改造和打包脚本。
- 已新增移动端能力模型、权限模型、后台同步模型，以及前端 capability-driven UI。
- 已新增 `src-tauri/src/mobile_bridge.rs`。
- 已新增:
  - `src-tauri/tauri.android.conf.json`
  - `src-tauri/tauri.ios.conf.json`
  - `src-tauri/Info.ios.plist`
  - `scripts/configure-android-project.mjs`
  - `src-tauri/capabilities/mobile.json`
  - `src-tauri/mobile-bridge/...`
- 已生成 Android 原生工程:
  - `src-tauri/gen/android`
- 已修正 Tauri 构建前端命令，避免 Android 构建子进程找不到 `pnpm`:
  - `src-tauri/tauri.conf.json`
  - `beforeDevCommand` 改为 `corepack pnpm dev`
  - `beforeBuildCommand` 改为 `corepack pnpm build`
- 已修正 Android 首次 Rust 编译时报错:
  - `src-tauri/src/mobile_bridge.rs`
  - `src-tauri/src/lib.rs`
  - 补上了 `tauri::Manager` trait 的导入
- 已补充一个最小 Android 端配置资产文件:
  - `src-tauri/gen/android/app/src/main/assets/tauri.conf.json`

### 本机环境改动

- 已确认 Rust 可用，并配置国内镜像:
  - `C:\Users\12285\.cargo\config.toml`
- 已设置:
  - `RUSTUP_DIST_SERVER=https://rsproxy.cn`
  - `RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup`
- 已安装 Android Rust targets:
  - `aarch64-linux-android`
  - `armv7-linux-androideabi`
  - `i686-linux-android`
  - `x86_64-linux-android`
- 已安装 Android Studio:
  - `C:\Program Files\Android\Android Studio`
- 已安装 Android SDK 组件:
  - `platform-tools`
  - `platforms;android-35`
  - `build-tools;35.0.1`
  - `ndk;27.0.12077973`
  - `cmake;3.22.1`
- 已安装 Visual Studio 2022 Build Tools + C++ workload。
- 已把 Android 相关变量写入 PowerShell profile:
  - `C:\Users\12285\OneDrive\文档\WindowsPowerShell\Microsoft.PowerShell_profile.ps1`
- 已设置用户环境变量:
  - `ANDROID_HOME`
  - `ANDROID_SDK_ROOT`
  - `NDK_HOME`
  - `ANDROID_JAVA_HOME`

## 已验证通过的步骤

- `corepack pnpm install --frozen-lockfile`
- `corepack pnpm check`
- `corepack pnpm build`
- `corepack pnpm tauri android init --ci --skip-targets-install`
- Android `release` Rust 编译已经成功跑通过一次
  - 但在 Tauri CLI 把 `.so` 链接到 `jniLibs` 时被 Windows 符号链接权限拦住
- Android `debug` Rust 编译也已经手工跑通:
  - 输出文件:
  - `src-tauri/target/aarch64-linux-android/debug/librelayclip_lib.so`

## 当前卡点

当前没有卡在代码编译本身，主要卡在两件事:

1. Tauri CLI 在 Windows 上尝试把 `.so` symlink 到 `src-tauri/gen/android/app/src/main/jniLibs/...`
   - 当前系统未开启 Developer Mode
   - 报错是: `Creation symbolic link is not allowed for this system`

2. 我开始改走“手工复制 `.so` + 直接跑 Gradle”这条路径时
   - `gradlew.bat` 需要下载 `gradle-8.14.3-bin.zip`
   - Java 侧下载超时
   - 我随后改成用 `curl.exe` 手工下载 Gradle zip
   - 这一步被你主动中断了，所以要从这里继续

## 当前已经准备好的手工打包状态

以下内容已经就位:

- Android 工程已存在:
  - `src-tauri/gen/android`
- Android 资产配置已存在:
  - `src-tauri/gen/android/app/src/main/assets/tauri.conf.json`
- arm64 的 debug `.so` 已存在:
  - `src-tauri/target/aarch64-linux-android/debug/librelayclip_lib.so`
- `.so` 已复制到:
  - `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/librelayclip_lib.so`

也就是说，下次继续时，不需要再重新装 Rust / Android Studio / SDK / NDK / VS Build Tools。

## 下次建议继续顺序

### 目标

先做出一个可安装测试的 Android `arm64 debug APK`。

### 建议步骤

1. 先手工下载 Gradle 8.14.3 zip
   - URL:
   - `https://services.gradle.org/distributions/gradle-8.14.3-bin.zip`

2. 解压到本地，例如:
   - `C:\Users\12285\AppData\Local\Programs\Gradle\gradle-8.14.3`

3. 不走 `gradlew.bat`，直接用本地 `gradle.bat` 构建

4. 在 `src-tauri/gen/android` 目录执行:
   - `assembleArm64Debug`
   - 并跳过会触发 symlink 的任务:
   - `-x rustBuildArm64Debug`

### 推荐命令模板

下面这条命令是下次优先尝试的:

```powershell
cmd /v:on /c "set ""JAVA_HOME=C:\Program Files\Android\Android Studio\jbr"" && set ""ANDROID_HOME=%LOCALAPPDATA%\Android\Sdk"" && set ""ANDROID_SDK_ROOT=%LOCALAPPDATA%\Android\Sdk"" && set ""NDK_HOME=%LOCALAPPDATA%\Android\Sdk\ndk\27.0.12077973"" && cd /d C:\Users\12285\IdeaProjects\relay-clip\src-tauri\gen\android && C:\Users\12285\AppData\Local\Programs\Gradle\gradle-8.14.3\bin\gradle.bat assembleArm64Debug -x rustBuildArm64Debug --stacktrace"
```

## 如果上面还失败，优先检查这些点

### 1. Gradle 是否已下载并解压

- 检查:
  - `C:\Users\12285\AppData\Local\Programs\Gradle\gradle-8.14.3\bin\gradle.bat`

### 2. `.so` 是否还在

- 检查:
  - `src-tauri/target/aarch64-linux-android/debug/librelayclip_lib.so`
  - `src-tauri/gen/android/app/src/main/jniLibs/arm64-v8a/librelayclip_lib.so`

### 3. Android 资产配置是否还在

- 检查:
  - `src-tauri/gen/android/app/src/main/assets/tauri.conf.json`

### 4. 如果想走 Tauri 官方完整 `tauri android build --apk`

需要先解决 Windows symlink 权限问题，二选一:

- 开启 Windows Developer Mode
- 或者后续把 Android 打包链路改成“copy `.so` 而不是 symlink”

## 重要说明

- 当前移动端 native bridge 仍然是 scaffold 级别，不是完整功能实现。
- 即使下次 APK 成功打出来，也更适合做“能否安装、UI 是否启动、基础流程是否可跑”的 smoke test。
- 文件分享 / 导出 / 原生剪贴板 / 原生发现等移动端能力还没有做完。
- iOS 这次没有继续推进到可本机打包，当前重点已收敛到 Android 测试包。

## 这次最关键的经验结论

- Android SDK / NDK / Rust target / VS Build Tools 现在都已经补齐了。
- 仓库代码已经进入“可以真实 Android 首编”的阶段。
- 当前剩余的不是大改代码，而是把 Windows 本机的 Android 打包闭环收好。
