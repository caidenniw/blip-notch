Unicode true
RequestExecutionLevel user

!ifndef VERSION
  !define VERSION "0.2.0"
!endif

Name "Blip"
OutFile "Blip_${VERSION}_x64-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\Blip"
InstallDirRegKey HKCU "Software\Blip" "InstallDir"

!include "MUI2.nsh"
!include "FileFunc.nsh"

!define MUI_ABORTWARNING
!define MUI_ICON "..\assets\icon.ico"
!define MUI_UNICON "..\assets\icon.ico"

!define MUI_FINISHPAGE_RUN "$INSTDIR\blip.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Jalankan Blip"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "Indonesian"
!insertmacro MUI_LANGUAGE "English"

Section "Install"
  DetailPrint "Menutup Blip jika sedang berjalan..."
  nsExec::Exec 'taskkill /F /IM blip.exe'

  SetOutPath "$INSTDIR"
  File "..\target\release\blip.exe"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  CreateShortcut "$SMPROGRAMS\Blip.lnk" "$INSTDIR\blip.exe" "" "$INSTDIR\blip.exe" 0

  WriteRegStr HKCU "Software\Blip" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "DisplayName" "Blip"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "Publisher" "Blip"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "DisplayIcon" "$INSTDIR\blip.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip" "NoRepair" 1
SectionEnd

Section "Uninstall"
  DetailPrint "Menutup Blip..."
  nsExec::Exec 'taskkill /F /IM blip.exe'

  Delete "$SMPROGRAMS\Blip.lnk"
  Delete "$INSTDIR\blip.exe"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Blip"
  DeleteRegKey HKCU "Software\Blip"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Blip"
SectionEnd
