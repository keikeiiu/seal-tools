; Seal Tools per-user installer.
;
; Build with:
;   makensis /DVERSION=3.0.14 /DPAYLOAD=<staged payload> /DOUTFILE=<installer.exe> install.nsi
;
; The install is per-user (no UAC). config/ and logs/ are user data: an upgrade never replaces them
; and the uninstaller keeps them. The portable zip remains the self-install path.

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"

!ifndef VERSION
  !error "VERSION is required (/DVERSION=x.y.z)"
!endif
!ifndef PAYLOAD
  !error "PAYLOAD is required (/DPAYLOAD=<staged payload>|<payload>)"
!endif
!ifndef OUTFILE
  !error "OUTFILE is required (/DOUTFILE=<installer.exe>)"
!endif

Name "Seal Tools v${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\SealTools"
InstallDirRegKey HKCU "Software\SealTools" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show

!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\SealTools.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Start Seal Tools"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\SealTools"

Section "Seal Tools (required)" SecMain
  SectionIn RO
  SetOutPath "$INSTDIR"

  ; Program files only. config/ and logs/ are never part of an upgrade replacement.
  File "${PAYLOAD}\SealTools.exe"
  File "${PAYLOAD}\seal-core.exe"
  File "${PAYLOAD}\DirectML.dll"
  File "${PAYLOAD}\flutter_windows.dll"
  File "${PAYLOAD}\native_assets.json"
  File "${PAYLOAD}\build-info.json"

  SetOutPath "$INSTDIR\data"
  File /r "${PAYLOAD}\data\*.*"
  SetOutPath "$INSTDIR\docs"
  File /r "${PAYLOAD}\docs\*.*"
  SetOutPath "$INSTDIR\models"
  File /r "${PAYLOAD}\models\*.*"

  ; Fresh install: seed config templates. Upgrade: keep the existing config untouched.
  IfFileExists "$INSTDIR\config\README.md" doneConfig
    SetOutPath "$INSTDIR\config"
    File /r "${PAYLOAD}\config\*.*"
  doneConfig:
  SetOutPath "$INSTDIR"

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  WriteRegStr HKCU "Software\SealTools" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "Seal Tools v${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\SealTools.exe"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Seal Tools"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Desktop shortcut" SecDesktop
  CreateShortcut "$DESKTOP\Seal Tools.lnk" "$INSTDIR\SealTools.exe"
SectionEnd

Section "Start menu shortcut" SecStartMenu
  CreateDirectory "$SMPROGRAMS\Seal Tools"
  CreateShortcut "$SMPROGRAMS\Seal Tools\Seal Tools.lnk" "$INSTDIR\SealTools.exe"
  CreateShortcut "$SMPROGRAMS\Seal Tools\Uninstall Seal Tools.lnk" "$INSTDIR\Uninstall.exe"
SectionEnd

Section /o "Start Seal Tools with Windows" SecAutostart
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SealTools" '"$INSTDIR\SealTools.exe"'
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecMain} "The Seal Tools program, OCR models and documentation."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "Add a shortcut to the desktop."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecStartMenu} "Add shortcuts to the Start menu."
  !insertmacro MUI_DESCRIPTION_TEXT ${SecAutostart} "Start Seal Tools automatically when you sign in."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Delete "$DESKTOP\Seal Tools.lnk"
  Delete "$SMPROGRAMS\Seal Tools\Seal Tools.lnk"
  Delete "$SMPROGRAMS\Seal Tools\Uninstall Seal Tools.lnk"
  RMDir "$SMPROGRAMS\Seal Tools"

  Delete "$INSTDIR\SealTools.exe"
  Delete "$INSTDIR\seal-core.exe"
  Delete "$INSTDIR\DirectML.dll"
  Delete "$INSTDIR\flutter_windows.dll"
  Delete "$INSTDIR\native_assets.json"
  Delete "$INSTDIR\build-info.json"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR\data"
  RMDir /r "$INSTDIR\docs"
  RMDir /r "$INSTDIR\models"

  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "SealTools"
  DeleteRegKey HKCU "${UNINST_KEY}"
  DeleteRegKey HKCU "Software\SealTools"

  ; config/ and logs/ are deliberately kept. The user can remove them by hand.
  RMDir "$INSTDIR"

  IfSilent uninstallDone
  MessageBox MB_ICONINFORMATION|MB_OK "Seal Tools was removed. Your config/ and logs/ folders were kept in:$INSTDIR"
  uninstallDone:
SectionEnd
