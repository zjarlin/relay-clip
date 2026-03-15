import fs from 'node:fs'
import path from 'node:path'

const root = process.cwd()
const projectYamlPath = path.join(root, 'src-tauri', 'gen', 'apple', 'project.yml')
const projectPbxprojPath = path.join(
  root,
  'src-tauri',
  'gen',
  'apple',
  'relayclip.xcodeproj',
  'project.pbxproj',
)

const oldCommand = 'pnpm tauri ios xcode-script'
const newCommand = 'pnpm --dir ../../.. exec tauri ios xcode-script'

function patchFile(filePath) {
  if (!fs.existsSync(filePath)) {
    return
  }

  const current = fs.readFileSync(filePath, 'utf8')
  if (!current.includes(oldCommand) || current.includes(newCommand)) {
    return
  }

  const next = current.replaceAll(oldCommand, newCommand)
  fs.writeFileSync(filePath, next)
}

patchFile(projectYamlPath)
patchFile(projectPbxprojPath)
