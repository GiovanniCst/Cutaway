// Builds the two packages of the native build: the portable zip and the setup.
//
// One command, because a release assembled by hand is a release with one file
// from yesterday in it. What it produces, under dist/2.0:
//
//   Cutaway/                    the two executables and the licence
//   Cutaway-portable.zip        unzip anywhere and run
//   Cutaway-Setup.exe           per-user install, no elevation
//
// The checks along the way are the ones that have actually gone wrong. The two
// crates must declare the same version, or the About boxes disagree with each
// other. Neither executable may import the Visual C++ redistributable, which is
// not part of Windows: that one shipped, silently, until a dumpbin at packaging
// time found it.

import { spawnSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')
const out = path.join(root, 'dist', '2.0')
const payload = path.join(out, 'Cutaway')

const stop = (message) => {
  console.error(message)
  process.exit(1)
}

const run = (command, args, options = {}) => {
  const done = spawnSync(command, args, { stdio: 'inherit', ...options })
  if (done.status !== 0) stop(`${path.basename(command)} è uscito con ${done.status}`)
}

const versionOf = (crate) => {
  const manifest = readFileSync(path.join(root, crate, 'Cargo.toml'), 'utf-8')
  // The first version= line, which is the package's own: the dependency
  // versions further down would otherwise match first.
  return manifest.match(/^version = "([^"]+)"/m)?.[1]
}

const version = versionOf('editor-rs')
if (!version) stop("Nessuna versione in editor-rs/Cargo.toml")
const agentVersion = versionOf('agent-rs')
if (agentVersion !== version) {
  stop(`L'editor dichiara ${version}, l'agente ${agentVersion}`)
}

console.log(`\nCutaway ${version}\n`)

for (const crate of ['editor-rs', 'agent-rs']) {
  console.log(`--- ${crate}`)
  run('cargo', ['build', '--release'], { cwd: path.join(root, crate), shell: true })
}

mkdirSync(payload, { recursive: true })
const files = [
  ['editor-rs/target/release/Cutaway.exe', 'Cutaway.exe'],
  ['agent-rs/target/release/CutawayAgent.exe', 'CutawayAgent.exe'],
  ['LICENSE', 'LICENSE'],
  ['NOTICE', 'NOTICE'],
]
for (const [from, to] of files) {
  const source = path.join(root, from)
  if (!existsSync(source)) stop(`Manca ${from}`)
  copyFileSync(source, path.join(payload, to))
}

// Nothing outside Windows may be needed to start these.
//
// The msvc target links VCRUNTIME140.dll unless the C runtime is asked for
// statically, and that DLL is the Visual C++ redistributable: on a machine that
// never had it the program does not start, and Windows says only that a DLL is
// missing. Read out of the import table by looking for the name in the file,
// which needs no tool installed to be true.
for (const exe of ['Cutaway.exe', 'CutawayAgent.exe']) {
  const bytes = readFileSync(path.join(payload, exe)).toString('latin1')
  for (const forbidden of ['VCRUNTIME140', 'MSVCP140', 'api-ms-win-crt-']) {
    if (bytes.includes(forbidden)) {
      stop(
        `${exe} importa ${forbidden}: manca -C target-feature=+crt-static ` +
          `in ${exe === 'Cutaway.exe' ? 'editor-rs' : 'agent-rs'}/.cargo/config.toml`,
      )
    }
  }
}

// The icon and the version, read back from the file rather than assumed: the
// resource is compiled by a build script that is allowed to fail without
// stopping the build, so this is where a missing one is caught.
const declared = spawnSync(
  'powershell.exe',
  [
    '-NoProfile',
    '-Command',
    `$i = (Get-Item '${path.join(payload, 'Cutaway.exe')}').VersionInfo; ` +
      `Add-Type -AssemblyName System.Drawing; ` +
      `$c = [System.Drawing.Icon]::ExtractAssociatedIcon('${path.join(payload, 'Cutaway.exe')}').Width; ` +
      `"$($i.FileVersion)|$c"`,
  ],
  { encoding: 'utf-8' },
).stdout?.trim()
const [fileVersion, iconWidth] = (declared ?? '|').split('|')
if (fileVersion !== `${version}.0` && fileVersion !== version) {
  stop(`Cutaway.exe dichiara "${fileVersion}", il manifesto ${version}`)
}
if (!Number(iconWidth)) stop("Cutaway.exe non ha un'icona")

// The portable. A zip and not a single self-extracting exe: the agent has to be
// a real file beside the editor for the editor to find it, so unpacking is the
// installation and there is nothing clever to go wrong.
//
// The name carries no version, for the same reason the installer's does not: a
// release page serves its assets at /releases/latest/download/<name>, and a link
// built on that address only keeps working while the name stays still.
const zip = path.join(out, 'Cutaway-portable.zip')
run('powershell.exe', [
  '-NoProfile',
  '-Command',
  `Compress-Archive -Path '${payload}' -DestinationPath '${zip}' -Force`,
])

// The setup.
const candidates = [
  path.join(process.env.LOCALAPPDATA ?? '', 'Programs', 'Inno Setup 6', 'ISCC.exe'),
  'C:\\Program Files (x86)\\Inno Setup 6\\ISCC.exe',
  'C:\\Program Files\\Inno Setup 6\\ISCC.exe',
]
const iscc = candidates.find((candidate) => existsSync(candidate))
if (!iscc) {
  stop(
    'Inno Setup non trovato. Installalo con:\n' +
      '  winget install -e --id JRSoftware.InnoSetup --scope user',
  )
}
run(iscc, [`/DAppVersion=${version}`, path.join(root, 'installer', 'cutaway-2.0.iss')])

const setup = path.join(out, 'Cutaway-Setup.exe')
if (!existsSync(setup)) stop('ISCC ha finito senza produrre il setup')

const size = (file) => `${(statSync(file).size / 1024 / 1024).toFixed(2)} MB`
console.log(`
${path.join(payload, 'Cutaway.exe')}        ${size(path.join(payload, 'Cutaway.exe'))}
${path.join(payload, 'CutawayAgent.exe')}   ${size(path.join(payload, 'CutawayAgent.exe'))}
${zip}   ${size(zip)}
${setup}   ${size(setup)}
`)
