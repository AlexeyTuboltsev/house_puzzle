; packaging/windows/installer.iss — Inno Setup config for ai-validate.
;
; Builds a single-file ai-validate-<ver>-setup.exe that:
;   - locates each installed Adobe Illustrator version via registry,
;   - drops the bundled .jsx into <install>\Presets\en_US\Scripts\,
;   - covers all detected versions in one pass (artist may have 2022
;     + 2024 side-by-side),
;   - adds an uninstaller that removes the .jsx from each location.
;
; Build:
;   iscc /DAppVersion=0.1.0 /DBundleSrc=..\..\dist\ai-validate-0.1.0.jsx installer.iss
; CI sets both via /D flags (see release workflow).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#ifndef BundleSrc
  #define BundleSrc "..\..\dist\ai-validate-" + AppVersion + ".jsx"
#endif

[Setup]
AppId={{B3F1E1A0-1D2A-4C8B-9C6E-AI-VALIDATE}}
AppName=ai-validate for Adobe Illustrator
AppVersion={#AppVersion}
AppPublisher=AlexeyTuboltsev
AppPublisherURL=https://github.com/AlexeyTuboltsev/house_puzzle
DefaultDirName={autopf}\ai-validate
DefaultGroupName=ai-validate
DisableDirPage=yes
DisableProgramGroupPage=yes
PrivilegesRequired=admin
; Run the installer as a 64-bit process on 64-bit Windows so HKLM
; reads the native registry view. Without this, HKLM points at
; WOW6432Node and we miss every modern Illustrator install (Adobe
; only ships Illustrator as 64-bit on Windows since CC 2018).
ArchitecturesInstallIn64BitMode=x64compatible
OutputBaseFilename=ai-validate-{#AppVersion}-setup
OutputDir=..\..\dist
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
UninstallDisplayName=ai-validate {#AppVersion}

[Files]
; Stage the bundle into {app} so the uninstaller knows where it came
; from — the [Code] section copies it into each detected Illustrator
; install during install, and removes those copies on uninstall.
Source: "{#BundleSrc}"; DestDir: "{app}"; DestName: "ai-validate.jsx"; Flags: ignoreversion

[Code]
const
  // Adobe registers each installed Illustrator version under a
  // separate key. We walk HKLM\SOFTWARE\Adobe\Adobe Illustrator and
  // grab every subkey that has an InstallPath value. HKLM64 is
  // explicit so we read the native registry view even if the
  // installer ever runs in 32-bit mode (older Win or legacy ISCC).
  ADOBE_REG_ROOT = 'SOFTWARE\Adobe\Adobe Illustrator';

var
  TargetDirs: TArrayOfString;

procedure AddTargetDir(Dir: string);
begin
  SetArrayLength(TargetDirs, GetArrayLength(TargetDirs) + 1);
  TargetDirs[GetArrayLength(TargetDirs) - 1] := Dir;
end;

procedure DiscoverIllustratorTargets();
var
  Subkeys: TArrayOfString;
  LocaleDirs: TArrayOfString;
  i, j: Integer;
  InstallPath, PresetsDir, CandidateScripts: string;
  FoundLocale: Boolean;
  FindRec: TFindRec;
begin
  SetArrayLength(TargetDirs, 0);

  // Try the 64-bit view first (Adobe ships 64-bit only on Win10/11);
  // fall back to the 32-bit view in case some older install lives
  // in WOW6432Node.
  if not RegGetSubkeyNames(HKLM64, ADOBE_REG_ROOT, Subkeys) then
    if not RegGetSubkeyNames(HKLM32, ADOBE_REG_ROOT, Subkeys) then
      exit;

  for i := 0 to GetArrayLength(Subkeys) - 1 do
  begin
    if not RegQueryStringValue(HKLM64, ADOBE_REG_ROOT + '\' + Subkeys[i], 'InstallPath', InstallPath) then
      if not RegQueryStringValue(HKLM32, ADOBE_REG_ROOT + '\' + Subkeys[i], 'InstallPath', InstallPath) then
        continue;
    if InstallPath = '' then continue;

    // Illustrator places scripts under <install>\Presets\<locale>\Scripts
    // — the locale folder is en_US, de_DE, ja_JP, fr_FR, etc. depending
    // on the artist's chosen UI language. Enumerate Presets\* and pick
    // every subfolder that actually has a Scripts dir.
    PresetsDir := AddBackslash(InstallPath) + 'Presets';
    if not DirExists(PresetsDir) then
      continue;

    SetArrayLength(LocaleDirs, 0);
    if FindFirst(AddBackslash(PresetsDir) + '*', FindRec) then
    try
      repeat
        if (FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0 then
          if (FindRec.Name <> '.') and (FindRec.Name <> '..') then
          begin
            CandidateScripts := AddBackslash(PresetsDir) + FindRec.Name + '\Scripts';
            if DirExists(CandidateScripts) then
              AddTargetDir(CandidateScripts);
          end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  i: Integer;
  src, dst: string;
begin
  if CurStep = ssPostInstall then
  begin
    DiscoverIllustratorTargets();
    if GetArrayLength(TargetDirs) = 0 then
    begin
      MsgBox('No Adobe Illustrator installation was detected.' + #13#10 +
             'You can install ai-validate.jsx manually:' + #13#10 +
             'File > Scripts > Other Script... → ' + ExpandConstant('{app}\ai-validate.jsx'),
             mbInformation, MB_OK);
      exit;
    end;
    src := ExpandConstant('{app}\ai-validate.jsx');
    for i := 0 to GetArrayLength(TargetDirs) - 1 do
    begin
      dst := AddBackslash(TargetDirs[i]) + 'ai-validate.jsx';
      if not FileCopy(src, dst, False) then
        Log('failed to copy to ' + dst);
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  i: Integer;
  dst: string;
begin
  if CurUninstallStep = usUninstall then
  begin
    DiscoverIllustratorTargets();
    for i := 0 to GetArrayLength(TargetDirs) - 1 do
    begin
      dst := AddBackslash(TargetDirs[i]) + 'ai-validate.jsx';
      if FileExists(dst) then DeleteFile(dst);
    end;
  end;
end;
