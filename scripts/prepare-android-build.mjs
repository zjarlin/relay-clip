import { prepareAndroidBuild } from './android-build-utils.mjs'

const prepared = prepareAndroidBuild({ profile: 'debug', buildFrontend: false })

console.log(`Prepared Android debug build in ${prepared.androidProjectDir}`)
console.log(`Copied native library to ${prepared.copiedSoPath}`)
