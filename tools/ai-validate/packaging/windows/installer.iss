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

// Add a Scripts dir to TargetDirs only if it's not already there
// (registry + filesystem discovery may both find the same install).
procedure AddTargetDirUnique(Dir: string);
var
  i: Integer;
begin
  for i := 0 to GetArrayLength(TargetDirs) - 1 do
    if CompareText(TargetDirs[i], Dir) = 0 then exit;
  AddTargetDir(Dir);
end;

// Walk <install>\Presets\* and add every <locale>\Scripts dir that
// actually exists. Locale subfolder is en_US, de_DE, ja_JP, ru_RU,
// fr_FR, etc. depending on the artist's UI language; we don't
// hardcode any of them.
procedure AddPresetsLocaleDirs(InstallPath: string);
var
  PresetsDir: string;
  FindRec: TFindRec;
  CandidateScripts: string;
begin
  if InstallPath = '' then exit;
  PresetsDir := AddBackslash(InstallPath) + 'Presets';
  if not DirExists(PresetsDir) then exit;
  if FindFirst(AddBackslash(PresetsDir) + '*', FindRec) then
  try
    repeat
      if ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
          and (FindRec.Name <> '.') and (FindRec.Name <> '..') then
      begin
        CandidateScripts := AddBackslash(PresetsDir) + FindRec.Name + '\Scripts';
        if DirExists(CandidateScripts) then
          AddTargetDirUnique(CandidateScripts);
      end;
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

// For a given registry view, walk every subkey of
// SOFTWARE\Adobe\Adobe Illustrator and collect Scripts dirs from
// each install's Presets\<locale> folders.
procedure DiscoverViaRegistry(RootKey: Integer);
var
  Subkeys: TArrayOfString;
  i: Integer;
  InstallPath: string;
begin
  if not RegGetSubkeyNames(RootKey, ADOBE_REG_ROOT, Subkeys) then exit;
  for i := 0 to GetArrayLength(Subkeys) - 1 do
  begin
    if RegQueryStringValue(RootKey, ADOBE_REG_ROOT + '\' + Subkeys[i], 'InstallPath', InstallPath) then
      AddPresetsLocaleDirs(InstallPath);
  end;
end;

// Filesystem fallback: scan C:\Program Files\Adobe\ for any folder
// matching `Adobe Illustrator*` and treat it as an Illustrator
// install. Necessary when the registry is missing / mangled (Adobe
// Cleaner ran, Creative Cloud reset, custom install path, stub
// install hasn't populated the keys yet, etc.) but the app is
// actually installed on disk.
procedure DiscoverViaFilesystem(AdobeRoot: string);
var
  FindRec: TFindRec;
  CandidateDir: string;
begin
  if not DirExists(AdobeRoot) then exit;
  if FindFirst(AddBackslash(AdobeRoot) + 'Adobe Illustrator*', FindRec) then
  try
    repeat
      if ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
          and (FindRec.Name <> '.') and (FindRec.Name <> '..') then
      begin
        CandidateDir := AddBackslash(AdobeRoot) + FindRec.Name;
        AddPresetsLocaleDirs(CandidateDir);
      end;
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

procedure DiscoverIllustratorTargets();
begin
  SetArrayLength(TargetDirs, 0);

  // 1. Native (64-bit) registry view — where Illustrator 2018+
  //    actually registers itself.
  DiscoverViaRegistry(HKLM64);
  // 2. WOW6432Node fallback for any legacy 32-bit registration.
  DiscoverViaRegistry(HKLM32);
  // 3. Per-user fallback — Creative Cloud "Install for me" puts
  //    Illustrator under HKCU instead of HKLM.
  DiscoverViaRegistry(HKCU);
  // 4. Filesystem fallback in case the registry is missing /
  //    incomplete but the app is actually on disk.
  DiscoverViaFilesystem(ExpandConstant('{commonpf64}\Adobe'));
  DiscoverViaFilesystem(ExpandConstant('{commonpf32}\Adobe'));
end;

function YesNo(B: Boolean): string;
begin
  if B then Result := 'yes' else Result := 'no';
end;

function GetWindowsVersionString(): string;
var
  ProductName, BuildNumber, DisplayVersion, ReleaseId: string;
begin
  if not RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'ProductName', ProductName) then
    if not RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'ProductName', ProductName) then
      ProductName := '<unknown>';
  if not RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'CurrentBuildNumber', BuildNumber) then
    if not RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'CurrentBuildNumber', BuildNumber) then
      BuildNumber := '<unknown>';
  RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'DisplayVersion', DisplayVersion);
  RegQueryStringValue(HKLM64, 'SOFTWARE\Microsoft\Windows NT\CurrentVersion', 'ReleaseId', ReleaseId);

  Result := ProductName + ' (build ' + BuildNumber;
  if DisplayVersion <> '' then
    Result := Result + ', ' + DisplayVersion
  else if ReleaseId <> '' then
    Result := Result + ', ' + ReleaseId;
  Result := Result + ')';
  if IsWin64 then
    Result := Result + ' [OS 64-bit]'
  else
    Result := Result + ' [OS 32-bit]';
end;

procedure DumpAdobeIllustratorRegistry(var Lines: string; const ViewName: string; const RootKey: Integer);
var
  Subkeys: TArrayOfString;
  i: Integer;
  InstallPath, PresetsDir: string;
  FindRec: TFindRec;
begin
  Lines := Lines + 'Registry ' + ViewName + '\' + ADOBE_REG_ROOT + ':' + #13#10;
  if not RegGetSubkeyNames(RootKey, ADOBE_REG_ROOT, Subkeys) then
  begin
    Lines := Lines + '  (key not present in this view)' + #13#10;
    exit;
  end;
  if GetArrayLength(Subkeys) = 0 then
  begin
    Lines := Lines + '  (key exists but has no subkeys)' + #13#10;
    exit;
  end;
  for i := 0 to GetArrayLength(Subkeys) - 1 do
  begin
    Lines := Lines + '  ' + Subkeys[i] + #13#10;
    if RegQueryStringValue(RootKey, ADOBE_REG_ROOT + '\' + Subkeys[i], 'InstallPath', InstallPath) then
      Lines := Lines + '    InstallPath = ' + InstallPath + #13#10
    else begin
      Lines := Lines + '    InstallPath = (value missing)' + #13#10;
      InstallPath := '';
    end;

    if InstallPath <> '' then
    begin
      PresetsDir := AddBackslash(InstallPath) + 'Presets';
      Lines := Lines + '    Presets dir exists: ' + YesNo(DirExists(PresetsDir)) + #13#10;
      if DirExists(PresetsDir) then
      begin
        if FindFirst(AddBackslash(PresetsDir) + '*', FindRec) then
        try
          repeat
            if ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
                and (FindRec.Name <> '.') and (FindRec.Name <> '..') then
            begin
              Lines := Lines + '      Presets\' + FindRec.Name
                + '\Scripts dir: '
                + YesNo(DirExists(AddBackslash(PresetsDir) + FindRec.Name + '\Scripts'))
                + #13#10;
            end;
          until not FindNext(FindRec);
        finally
          FindClose(FindRec);
        end;
      end;
    end;
  end;
end;

procedure DumpAdobeRoot(var Lines: string; const ViewName: string; const RootKey: Integer);
var
  Subkeys: TArrayOfString;
  i: Integer;
begin
  Lines := Lines + 'Registry ' + ViewName + '\SOFTWARE\Adobe (subkeys, raw):' + #13#10;
  if not RegGetSubkeyNames(RootKey, 'SOFTWARE\Adobe', Subkeys) then
  begin
    Lines := Lines + '  (SOFTWARE\Adobe not present)' + #13#10;
    exit;
  end;
  if GetArrayLength(Subkeys) = 0 then
  begin
    Lines := Lines + '  (no subkeys)' + #13#10;
    exit;
  end;
  for i := 0 to GetArrayLength(Subkeys) - 1 do
    Lines := Lines + '  ' + Subkeys[i] + #13#10;
end;

procedure DumpAppPaths(var Lines: string);
var
  Path: string;
begin
  Lines := Lines + 'App Paths\Illustrator.exe:' + #13#10;
  if RegQueryStringValue(HKLM64,
      'SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\Illustrator.exe',
      '', Path) then
    Lines := Lines + '  HKLM64 (default) = ' + Path + #13#10
  else
    Lines := Lines + '  HKLM64: not found' + #13#10;
  if RegQueryStringValue(HKLM32,
      'SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\Illustrator.exe',
      '', Path) then
    Lines := Lines + '  HKLM32 (default) = ' + Path + #13#10
  else
    Lines := Lines + '  HKLM32: not found' + #13#10;
end;

procedure DumpFilesystemScan(var Lines: string; const RootName, AdobeRoot: string);
var
  FindRec: TFindRec;
  CandidateDir, PresetsDir: string;
  PresetsRec: TFindRec;
begin
  Lines := Lines + 'Filesystem scan: ' + RootName + ' = ' + AdobeRoot + #13#10;
  if not DirExists(AdobeRoot) then
  begin
    Lines := Lines + '  (not present)' + #13#10;
    exit;
  end;
  if not FindFirst(AddBackslash(AdobeRoot) + 'Adobe Illustrator*', FindRec) then
  begin
    Lines := Lines + '  (no "Adobe Illustrator*" subdirs)' + #13#10;
    exit;
  end;
  try
    repeat
      if ((FindRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
          and (FindRec.Name <> '.') and (FindRec.Name <> '..') then
      begin
        CandidateDir := AddBackslash(AdobeRoot) + FindRec.Name;
        Lines := Lines + '  ' + FindRec.Name + #13#10;
        PresetsDir := AddBackslash(CandidateDir) + 'Presets';
        Lines := Lines + '    Presets dir exists: ' + YesNo(DirExists(PresetsDir)) + #13#10;
        if DirExists(PresetsDir) then
        begin
          if FindFirst(AddBackslash(PresetsDir) + '*', PresetsRec) then
          try
            repeat
              if ((PresetsRec.Attributes and FILE_ATTRIBUTE_DIRECTORY) <> 0)
                  and (PresetsRec.Name <> '.') and (PresetsRec.Name <> '..') then
              begin
                Lines := Lines + '      Presets\' + PresetsRec.Name
                  + '\Scripts dir: '
                  + YesNo(DirExists(AddBackslash(PresetsDir) + PresetsRec.Name + '\Scripts'))
                  + #13#10;
              end;
            until not FindNext(PresetsRec);
          finally
            FindClose(PresetsRec);
          end;
        end;
      end;
    until not FindNext(FindRec);
  finally
    FindClose(FindRec);
  end;
end;

function BuildDiagnosticReport(): string;
var
  Lines: string;
begin
  Lines := '== ai-validate installer diagnostic ==' + #13#10;
  Lines := Lines + 'ai-validate version: {#AppVersion}' + #13#10;
  Lines := Lines + 'Installer 64-bit mode: ' + YesNo(Is64BitInstallMode) + #13#10;
  Lines := Lines + 'Admin install mode:    ' + YesNo(IsAdminInstallMode) + #13#10;
  Lines := Lines + 'Windows: ' + GetWindowsVersionString() + #13#10;
  Lines := Lines + #13#10;
  DumpAdobeIllustratorRegistry(Lines, 'HKLM64', HKLM64);
  Lines := Lines + #13#10;
  DumpAdobeIllustratorRegistry(Lines, 'HKLM32', HKLM32);
  Lines := Lines + #13#10;
  DumpAdobeIllustratorRegistry(Lines, 'HKCU',   HKCU);
  Lines := Lines + #13#10;
  DumpAdobeRoot(Lines, 'HKLM64', HKLM64);
  Lines := Lines + #13#10;
  DumpAdobeRoot(Lines, 'HKLM32', HKLM32);
  Lines := Lines + #13#10;
  DumpAppPaths(Lines);
  Lines := Lines + #13#10;
  DumpFilesystemScan(Lines, 'commonpf64', ExpandConstant('{commonpf64}\Adobe'));
  Lines := Lines + #13#10;
  DumpFilesystemScan(Lines, 'commonpf32', ExpandConstant('{commonpf32}\Adobe'));
  Result := Lines;
end;

procedure ShowDiagnosticForm(const Report: string);
var
  Form: TSetupForm;
  Memo: TNewMemo;
  Hint: TNewStaticText;
  OkBtn: TNewButton;
begin
  Form := CreateCustomForm;
  try
    Form.Caption := 'ai-validate — Adobe Illustrator not detected';
    Form.ClientWidth := ScaleX(640);
    Form.ClientHeight := ScaleY(500);
    Form.Position := poScreenCenter;
    Form.BorderStyle := bsDialog;

    Hint := TNewStaticText.Create(Form);
    Hint.Parent := Form;
    Hint.Left := ScaleX(12);
    Hint.Top := ScaleY(8);
    Hint.AutoSize := False;
    Hint.WordWrap := True;
    Hint.Width := ScaleX(616);
    Hint.Height := ScaleY(76);
    Hint.Caption :=
      'No Adobe Illustrator installation could be detected. The script ' +
      'is staged at ' + ExpandConstant('{app}\ai-validate.jsx') + ' — you ' +
      'can install it manually via File > Scripts > Other Script...' + #13#10 +
      'To help fix the installer for your machine, please copy the ' +
      'diagnostic info below (click into the box, Ctrl+A, Ctrl+C) and ' +
      'send it back.';

    Memo := TNewMemo.Create(Form);
    Memo.Parent := Form;
    Memo.Left := ScaleX(12);
    Memo.Top := ScaleY(92);
    Memo.Width := ScaleX(616);
    Memo.Height := ScaleY(366);
    Memo.ScrollBars := ssBoth;
    Memo.ReadOnly := True;
    Memo.WantReturns := False;
    Memo.WordWrap := False;
    Memo.Font.Name := 'Consolas';
    Memo.Font.Size := 9;
    Memo.Lines.Text := Report;

    OkBtn := TNewButton.Create(Form);
    OkBtn.Parent := Form;
    OkBtn.Caption := 'Close';
    OkBtn.Width := ScaleX(80);
    OkBtn.Height := ScaleY(25);
    OkBtn.Left := ScaleX(548);
    OkBtn.Top := ScaleY(466);
    OkBtn.ModalResult := mrOk;
    OkBtn.Default := True;

    Form.ShowModal;
  finally
    Form.Free;
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
      ShowDiagnosticForm(BuildDiagnosticReport());
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
