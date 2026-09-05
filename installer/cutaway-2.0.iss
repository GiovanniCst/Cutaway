; The Windows installer for the native build: per-user, no elevation, an
; uninstaller, and the offer to open the image formats Cutaway reads with
; Cutaway.
;
; Built by `node tools/build-2.0.mjs`, which passes the version in.
;
; What is not here is the whole point of it. The 1.6 installer carried a
; WebView2 bootstrapper, downloaded a quarter of a gigabyte from Microsoft when
; the runtime was missing, and had a page of Pascal explaining the wait. The
; native build has no runtime: two executables that import nothing but Windows'
; own DLLs, and their C runtime is inside them. Two files go in, and that is the
; installation.
;
; The AppId is the 1.6's, on purpose: this is the same product one major version
; on, so it upgrades an existing install in place and keeps one entry in
; Add/Remove Programs rather than leaving two Cutaways on the machine.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "Cutaway"
#define AppExe "Cutaway.exe"
#define AgentExe "CutawayAgent.exe"
; The agent holds this while it is resident; the setup asks it to leave first.
; The name is the one the 1.6's C# agent used too, so this setup can stop the
; agent it is replacing.
#define AgentMutex "Local\Cutaway.Agent"

[Setup]
AppId={{3375651E-D54A-4AF1-873B-F4057DAE9C84}
AppName={#AppName}
AppVersion={#AppVersion}
AppPublisher=Giovanni J. Costantini
AppPublisherURL=https://github.com/GiovanniCst/Cutaway
DefaultDirName={autopf}\{#AppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist\2.0
OutputBaseFilename=Cutaway-Setup
SetupIconFile=..\assets\cutaway.ico
UninstallDisplayIcon={app}\{#AppExe}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "italian"; MessagesFile: "compiler:Languages\Italian.isl"

[CustomMessages]
english.AssocTask=Open the image formats Cutaway reads (%1) with Cutaway
italian.AssocTask=Apri con Cutaway i formati immagine che l'app legge (%1)
english.ProgDesc=Cutaway image
italian.ProgDesc=Immagine Cutaway
english.FinishShortcut=%n%nPress Ctrl+PrtSc or AltGr+PrtSc, from anywhere, to cut a piece of the screen.
italian.FinishShortcut=%n%nPremi Ctrl+Stamp (PrtSc) o AltGr+Stamp, da qualsiasi programma, per ritagliare lo schermo.
english.AgentRunning=Cutaway is still running in the background and its files cannot be replaced. Quit it from the Cutaway icon next to the clock, then run this setup again.
italian.AgentRunning=Cutaway è ancora in esecuzione in secondo piano e i suoi file non si possono sostituire. Chiudilo dall'icona Cutaway vicino all'orologio, poi riavvia questa installazione.

[Tasks]
Name: "fileassoc"; Description: "{cm:AssocTask,.png .jpg .jpeg .webp .bmp .gif .tif .tiff}"
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; Flags: unchecked

[InstallDelete]
; Upgrading from a 1.6 install: PyInstaller's payload folder, sixty megabytes of
; interpreter and libraries that nothing here needs any more. The onefile
; portable of that era does not live in here, so nothing else has to go.
Type: filesandordirs; Name: "{app}\_internal"

[Files]
Source: "..\dist\2.0\{#AppName}\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\2.0\{#AppName}\{#AgentExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\NOTICE"; DestDir: "{app}"; Flags: ignoreversion
; A second copy of the agent, never installed: extracted to {tmp} so the setup
; can ask a running agent to quit before replacing anything - including the
; agent of a portable copy, which lives somewhere else entirely.
Source: "..\dist\2.0\{#AppName}\{#AgentExe}"; Flags: dontcopy noencryption

[Icons]
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Written by the agent itself, not here: this line exists so that uninstalling
; takes it away again. ValueType none means "do not create it".
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "{#AppName}"; ValueType: none; Flags: uninsdeletevalue

; The app, findable by name from Run and from the shell.
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\{#AppExe}"; ValueType: string; ValueData: "{app}\{#AppExe}"; Flags: uninsdeletekey

; The document type every association points at.
Root: HKA; Subkey: "Software\Classes\{#AppName}.Image"; ValueType: string; ValueData: "{cm:ProgDesc}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#AppName}.Image\DefaultIcon"; ValueType: string; ValueData: "{app}\{#AppExe},0"
Root: HKA; Subkey: "Software\Classes\{#AppName}.Image\shell\open\command"; ValueType: string; ValueData: """{app}\{#AppExe}"" ""%1"""

; Registered application, so Cutaway shows up properly in Settings > Default apps.
Root: HKA; Subkey: "Software\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\{#AppName}\Capabilities"; ValueType: string; ValueName: "ApplicationDescription"; ValueData: "{cm:ProgDesc}"
Root: HKA; Subkey: "Software\RegisteredApplications"; ValueType: string; ValueName: "{#AppName}"; ValueData: "Software\{#AppName}\Capabilities"; Flags: uninsdeletevalue

; Always in "Open with", for every format the app reads.
Root: HKA; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jpg\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jpeg\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.webp\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.bmp\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.gif\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.tif\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.tiff\OpenWithProgids"; ValueType: string; ValueName: "{#AppName}.Image"; ValueData: ""; Flags: uninsdeletevalue

; The default handler, only where the task was left ticked.
Root: HKA; Subkey: "Software\Classes\.png"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jpg"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.jpeg"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.webp"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.bmp"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.gif"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.tif"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.tiff"; ValueType: string; ValueData: "{#AppName}.Image"; Tasks: fileassoc; Flags: uninsdeletevalue

; What the registered application claims to handle, same list.
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".png"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpg"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".jpeg"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".webp"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".bmp"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".gif"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tif"; ValueData: "{#AppName}.Image"
Root: HKA; Subkey: "Software\{#AppName}\Capabilities\FileAssociations"; ValueType: string; ValueName: ".tiff"; ValueData: "{#AppName}.Image"

[Run]
; nowait because the agent is resident and never returns: without it the wizard
; would sit on "Finishing installation" for good. And no skipifsilent, because a
; /VERYSILENT install has to end with the shortcut working too.
Filename: "{app}\{#AgentExe}"; Parameters: "--background"; Flags: nowait
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent

[UninstallRun]
; Before the files go: the agent is holding one of them.
Filename: "{app}\{#AgentExe}"; Parameters: "--quit"; RunOnceId: "StopAgent"; Flags: waituntilterminated

[UninstallDelete]
; What the agent writes while it runs, which the file list knows nothing about.
Type: filesandordirs; Name: "{localappdata}\{#AppName}\agent"
Type: filesandordirs; Name: "{localappdata}\{#AppName}\captures"

[Code]
{ Asks whatever agent is running to leave, and waits until it has. Run from the
  temporary folder rather than from the install folder: the one that needs
  stopping may belong to a portable copy, which keeps its agent under
  %LOCALAPPDATA% instead - or to a 1.6 install, whose agent is the C# one. The
  mutex and the quit event carry the same names in both, which is what makes
  one setup able to stop either.

  --quit waits for the process to be gone, which is what replacing its file
  needs to know.

  Note for anyone editing the comments here: a brace comment in Inno ends at the
  first closing brace, so a constant written inline would end it early and turn
  the rest of the paragraph into code. }
procedure StopAgent;
var
  ExitCode: Integer;
begin
  ExtractTemporaryFile('{#AgentExe}');
  Exec(ExpandConstant('{tmp}\{#AgentExe}'), '--quit', '', SW_HIDE,
       ewWaitUntilTerminated, ExitCode);
end;

{ The shortcut is what people forget they have, and nothing else on screen
  mentions it: no new page, one paragraph on the page that is already there. }
procedure CurPageChanged(CurPageID: Integer);
begin
  if CurPageID = wpFinished then
    WizardForm.FinishedLabel.Caption :=
      WizardForm.FinishedLabel.Caption + CustomMessage('FinishShortcut');
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  StopAgent;
  { A safety net, not the mechanism: if something still holds the mutex after
    --quit came back, replacing the files would fail halfway through instead. }
  if CheckForMutexes('{#AgentMutex}') then
    Result := CustomMessage('AgentRunning');
end;
