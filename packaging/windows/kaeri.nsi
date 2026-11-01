; Kaeri installer.
;
; Installs the binary, puts it on PATH, and adds a Start Menu shortcut. Both matter: Kaeri is
; a terminal program people will type, and also something they will look for in the Start Menu.
;
; The shortcut targets Windows Terminal rather than the binary directly. Launching a console
; program from a shortcut gets the legacy console host, which has no truecolor and no graphics
; protocol; wt.exe gets the modern one.
;
; Built by CI:  makensis -DVERSION=x.y.z -DSOURCE=<dir> kaeri.nsi

Unicode true
!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "WinMessages.nsh"   ; HWND_BROADCAST, WM_WININICHANGE
!include "WordFunc.nsh"      ; WordReplace, for taking ourselves back off PATH

!ifndef VERSION
	!define VERSION "0.0.0"
!endif
!ifndef SOURCE
	!define SOURCE "."
!endif

Name "Kaeri ${VERSION}"
OutFile "kaeri-${VERSION}-setup.exe"
InstallDir "$LOCALAPPDATA\Programs\kaeri"
InstallDirRegKey HKCU "Software\kaeri" "InstallDir"
RequestExecutionLevel user     ; per-user install, so no UAC prompt
SetCompressor /SOLID lzma

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "kaeri"
VIAddVersionKey "FileDescription" "An anime streaming TUI"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "MIT"

!define MUI_ICON "${SOURCE}\kaeri.ico"
!define MUI_UNICON "${SOURCE}\kaeri.ico"
!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_LICENSE "${SOURCE}\LICENSE"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"

Section "Kaeri (required)" SecCore
	SectionIn RO
	SetOutPath "$INSTDIR"
	File "${SOURCE}\kaeri.exe"
	File "${SOURCE}\kaeri.ico"
	File "${SOURCE}\LICENSE"
	File "${SOURCE}\README.md"
	SetOutPath "$INSTDIR\third-party"
	File /r "${SOURCE}\third-party\*"
	SetOutPath "$INSTDIR"

	WriteRegStr HKCU "Software\kaeri" "InstallDir" "$INSTDIR"
	WriteUninstaller "$INSTDIR\uninstall.exe"

	; Add/Remove Programs.
	!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\kaeri"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "kaeri"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
	WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\kaeri.ico"
	WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" "$INSTDIR\uninstall.exe"
	WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "kaeri"
	WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://kaeri.app"
	WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
	WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

Section "Start Menu shortcut" SecShortcut
	CreateDirectory "$SMPROGRAMS\kaeri"
	; Prefer Windows Terminal; fall back to running the binary directly where it is absent.
	${If} ${FileExists} "$LOCALAPPDATA\Microsoft\WindowsApps\wt.exe"
		CreateShortcut "$SMPROGRAMS\kaeri\kaeri.lnk" \
			"$LOCALAPPDATA\Microsoft\WindowsApps\wt.exe" \
			'--title kaeri "$INSTDIR\kaeri.exe"' \
			"$INSTDIR\kaeri.ico" 0
	${Else}
		CreateShortcut "$SMPROGRAMS\kaeri\kaeri.lnk" \
			"$INSTDIR\kaeri.exe" "" "$INSTDIR\kaeri.ico" 0
	${EndIf}
	CreateShortcut "$SMPROGRAMS\kaeri\Uninstall kaeri.lnk" "$INSTDIR\uninstall.exe"
SectionEnd

Section "Add to PATH" SecPath
	; Per-user PATH, read-modify-write. Registry only — the environment of already-open shells
	; cannot be changed, which is why the finish text says to open a new terminal.
	ReadRegStr $0 HKCU "Environment" "Path"
	${If} $0 == ""
		WriteRegExpandStr HKCU "Environment" "Path" "$INSTDIR"
	${Else}
		${If} $0 != "*$INSTDIR*"
			WriteRegExpandStr HKCU "Environment" "Path" "$0;$INSTDIR"
		${EndIf}
	${EndIf}
	SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
	!insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "The Kaeri binary."
	!insertmacro MUI_DESCRIPTION_TEXT ${SecShortcut} "A Start Menu entry that opens Kaeri in Windows Terminal."
	!insertmacro MUI_DESCRIPTION_TEXT ${SecPath} "Make `kaeri` runnable from any terminal."
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
	Delete "$INSTDIR\kaeri.exe"
	Delete "$INSTDIR\kaeri.ico"
	Delete "$INSTDIR\LICENSE"
	Delete "$INSTDIR\README.md"
	RMDir /r "$INSTDIR\third-party"
	Delete "$INSTDIR\uninstall.exe"
	RMDir "$INSTDIR"

	Delete "$SMPROGRAMS\kaeri\kaeri.lnk"
	Delete "$SMPROGRAMS\kaeri\Uninstall kaeri.lnk"
	RMDir "$SMPROGRAMS\kaeri"

	; Take ourselves back off PATH, leaving anything else the user added.
	ReadRegStr $0 HKCU "Environment" "Path"
	${WordReplace} "$0" ";$INSTDIR" "" "+" $1
	WriteRegExpandStr HKCU "Environment" "Path" "$1"
	SendMessage ${HWND_BROADCAST} ${WM_WININICHANGE} 0 "STR:Environment" /TIMEOUT=5000

	DeleteRegKey HKCU "Software\kaeri"
	DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\kaeri"
SectionEnd
