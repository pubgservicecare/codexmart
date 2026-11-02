; CODE X MART additions to the Windows installer (used by installer.nsi).
;
; When CODE X MART is uninstalled, a page asks what happens to the shop data:
;
;   Keep my data and credentials        default; also used for silent (/S) and update uninstalls
;   Delete all my data and credentials  needs two confirmations; silent: /S /DELETEDATA
;
; "Delete" removes, for the current Windows user:
;   %APPDATA%\CODE X MART              database, local backups, temporary files, backup history
;   %APPDATA%\com.codexmart.pos        application data, if any
;   %LOCALAPPDATA%\com.codexmart.pos   log files and the WebView2 profile (cache, local storage)
;
; It never removes:
;   - this computer's license record (HKCU\Software\CODE X MART Licensing and the
;     "CODE X MART Licensing" folders), so reinstalling does not start a new free trial;
;   - backup copies saved to a USB drive or another folder chosen by the user.
;
; Folders that cannot be deleted because a file is in use are listed to the user, reported
; with exit code 2 and scheduled for deletion at restart where Windows allows it. The shop
; data folder also gets a DELETE-PENDING.txt marker: CODE X MART finishes deleting it the
; next time it starts, before opening anything.

!include nsDialogs.nsh
!include LogicLib.nsh

!define CXM_DATA_DIR "$APPDATA\CODE X MART"

!macro CXM_UNINSTALL_DATA_PAGE
  Var CxmKeepRadio
  Var CxmDeleteRadio
  Var CxmBoldFont
  UninstPage custom un.CxmDataPage un.CxmDataLeave

  Function un.CxmDataPage
    ${If} $PassiveMode = 1
    ${OrIf} $UpdateMode = 1
      Abort
    ${EndIf}
    !insertmacro MUI_HEADER_TEXT "Your CODE X MART data" "Choose what happens to your shop data, login and backups on this computer."
    nsDialogs::Create 1018
    Pop $0
    ${If} $0 == error
      Abort
    ${EndIf}
    CreateFont $CxmBoldFont "$(^Font)" "$(^FontSize)" 700

    ${NSD_CreateRadioButton} 0 0 100% 12u "Keep my data and credentials"
    Pop $CxmKeepRadio
    SendMessage $CxmKeepRadio ${WM_SETFONT} $CxmBoldFont 1
    ${NSD_CreateLabel} 12u 13u -12u 34u "Your shop database, products, sales, purchases, customers, suppliers, login credentials, settings, backups and application data will remain on this computer. You can reinstall CODE X MART later and continue using your existing account and shop data."
    Pop $0

    ${NSD_CreateRadioButton} 0 52u 100% 12u "Delete all my data and credentials"
    Pop $CxmDeleteRadio
    SendMessage $CxmDeleteRadio ${WM_SETFONT} $CxmBoldFont 1
    ${NSD_CreateLabel} 12u 65u -12u 34u "This will permanently remove your CODE X MART shop data, login account, credentials, settings and locally stored backups from this computer. After reinstalling, you will need to create a new owner/admin account and set up the software again."
    Pop $0

    ${NSD_CreateLabel} 0 106u 100% 26u "In both cases, backup copies you saved to a USB drive or another folder are not touched, and this computer's CODE X MART license record is kept (reinstalling does not start a new free trial)."
    Pop $0

    ${If} $DeleteAppDataCheckboxState = 1
      ${NSD_Check} $CxmDeleteRadio
    ${Else}
      ${NSD_Check} $CxmKeepRadio
    ${EndIf}
    nsDialogs::Show
  FunctionEnd

  Function un.CxmDataLeave
    ${NSD_GetState} $CxmDeleteRadio $0
    ${If} $0 <> ${BST_CHECKED}
      StrCpy $DeleteAppDataCheckboxState 0
      Return
    ${EndIf}
    MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 "Are you sure you want to permanently delete all CODE X MART data and credentials?$\r$\n$\r$\nThis deletes the shop database (products, sales, purchases, customers, suppliers, expenses), every login account and password, the recovery key, all settings and the backups in ${CXM_DATA_DIR}\backups." IDYES cxm_confirm_again
    Abort
    cxm_confirm_again:
    MessageBox MB_YESNO|MB_ICONSTOP|MB_DEFBUTTON2 "Final confirmation$\r$\n$\r$\nDeleted data cannot be recovered unless you have a backup copy somewhere else, for example on a USB drive.$\r$\n$\r$\nDelete all CODE X MART data and credentials now?" IDYES cxm_confirmed
    Abort
    cxm_confirmed:
    StrCpy $DeleteAppDataCheckboxState 1
  FunctionEnd
!macroend

; Silent uninstall: /DELETEDATA chooses "Delete all my data and credentials".
!macro CXM_UNINIT
  ${GetOptions} $CMDLINE "/DELETEDATA" $R0
  ${IfNot} ${Errors}
    StrCpy $DeleteAppDataCheckboxState 1
  ${EndIf}
!macroend

; Deletes one folder, retrying while files are briefly locked. Folders still in
; use are scheduled for deletion at restart and collected in $R8.
!macro CXM_REMOVE_DIR path id
  StrCpy $R9 0
  cxm_retry_${id}:
    IfFileExists "${path}\*.*" 0 cxm_done_${id}
    ; Read-only files (for example a backup marked read-only) would block RMDir.
    nsExec::Exec '"$SYSDIR\attrib.exe" -R "${path}\*" /S /D'
    Pop $R7
    RMDir /r "${path}"
    IfFileExists "${path}\*.*" 0 cxm_deleted_${id}
    IntOp $R9 $R9 + 1
    IntCmp $R9 10 cxm_failed_${id} 0 cxm_failed_${id}
    Sleep 500
    Goto cxm_retry_${id}
  cxm_failed_${id}:
    RMDir /r /REBOOTOK "${path}"
    DetailPrint "Could not delete yet (a file is in use): ${path}"
    StrCpy $R8 "$R8$\r$\n    ${path}"
    Goto cxm_done_${id}
  cxm_deleted_${id}:
    DetailPrint "Deleted: ${path}"
  cxm_done_${id}:
!macroend

!macro CXM_DELETE_USER_DATA
  SetShellVarContext current
  StrCpy $R8 ""
  ; CODE X MART was closed just before (CheckIfAppIsRunning). Give Windows a
  ; moment to release the database file.
  Sleep 500
  !insertmacro CXM_REMOVE_DIR "${CXM_DATA_DIR}" data
  ; If the shop data could not all be deleted, CODE X MART finishes the job
  ; the next time it starts, before opening anything (see state.rs).
  IfFileExists "${CXM_DATA_DIR}\*.*" 0 +4
    FileOpen $R7 "${CXM_DATA_DIR}\DELETE-PENDING.txt" w
    FileWrite $R7 "The owner chose 'Delete all my data and credentials' when uninstalling CODE X MART, but some files were in use. CODE X MART deletes this folder the next time it starts."
    FileClose $R7
  !insertmacro CXM_REMOVE_DIR "$APPDATA\${BUNDLEID}" roaming
  !insertmacro CXM_REMOVE_DIR "$LOCALAPPDATA\${BUNDLEID}" local
  ${If} $R8 == ""
    DetailPrint "All CODE X MART shop data, accounts and local backups were deleted. This computer's license record was kept."
  ${Else}
    SetErrorLevel 2
    IfSilent +2
    MessageBox MB_OK|MB_ICONEXCLAMATION "Some CODE X MART data could not be deleted yet because files are in use:$R8$\r$\n$\r$\nThey will be deleted when Windows restarts. To finish now, close any program using these folders (for example a backup or sync tool) and delete them."
  ${EndIf}
!macroend

!macro CXM_KEPT_USER_DATA
  SetShellVarContext current
  DetailPrint "Your shop data, accounts and backups were kept in ${CXM_DATA_DIR}."
!macroend
