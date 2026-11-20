!include WinMessages.nsh
!include LogicLib.nsh
!ifndef CAI_ENV_KEY
  !define CAI_ENV_KEY "Environment"
!endif
!ifmacrondef CAI_BROADCAST
  !macro CAI_BROADCAST
    SendMessage ${HWND_BROADCAST} ${WM_SETTINGCHANGE} 0 "STR:Environment" /TIMEOUT=1000
  !macroend
!endif

!macro CAI_DEFINE_FIND PREFIX
Function ${PREFIX}CaiFind
  Exch $R1
  Exch
  Exch $R0
  Push $R2
  Push $R3
  Push $R4
  StrLen $R3 $R1
  StrCpy $R2 0
  loop:
    StrCpy $R4 $R0 $R3 $R2
    StrCmp $R4 "" notfound
    StrCmp $R4 $R1 done
    IntOp $R2 $R2 + 1
    Goto loop
  notfound:
    StrCpy $R2 -1
  done:
  Pop $R4
  Pop $R3
  Exch $R2
  Exch
  Pop $R0
  Exch
  Pop $R1
FunctionEnd
!macroend

!insertmacro CAI_DEFINE_FIND ""
!insertmacro CAI_DEFINE_FIND "un."

!macro CAI_FIND PREFIX HAYSTACK NEEDLE OUT
  Push "${HAYSTACK}"
  Push "${NEEDLE}"
  Call ${PREFIX}CaiFind
  Pop ${OUT}
!macroend

Function CaiAddToPath
  Push $0
  Push $1
  Push $2
  Push $3

  StrCpy $0 0
  StrCpy $3 0
  enum:
    ClearErrors
    EnumRegValue $1 HKCU "${CAI_ENV_KEY}" $0
    IfErrors enum_done
    StrCmp $1 "" enum_done
    StrCmp $1 "Path" 0 +3
      StrCpy $3 1
      Goto enum_done
    IntOp $0 $0 + 1
    Goto enum
  enum_done:

  ${If} $3 == 0
    WriteRegExpandStr HKCU "${CAI_ENV_KEY}" "Path" "$INSTDIR"
    Goto broadcast
  ${EndIf}

  ClearErrors
  ReadRegStr $1 HKCU "${CAI_ENV_KEY}" "Path"
  ${If} ${Errors}
    DetailPrint "Your Path is too long to change safely. Add $INSTDIR to it by hand to use clipboard-ai in a terminal."
    Goto end
  ${EndIf}
  ${If} $1 == ""
    WriteRegExpandStr HKCU "${CAI_ENV_KEY}" "Path" "$INSTDIR"
    Goto broadcast
  ${EndIf}

  !insertmacro CAI_FIND "" ";$1;" ";$INSTDIR;" $2
  ${If} $2 >= 0
    Goto end
  ${EndIf}
  !insertmacro CAI_FIND "" ";$1;" ";$INSTDIR\;" $2
  ${If} $2 >= 0
    Goto end
  ${EndIf}

  StrLen $2 $1
  StrLen $0 $INSTDIR
  IntOp $2 $2 + $0
  IntOp $2 $2 + 2
  ${If} $2 >= ${NSIS_MAX_STRLEN}
    DetailPrint "Your Path is too long to change safely. Add $INSTDIR to it by hand to use clipboard-ai in a terminal."
    Goto end
  ${EndIf}

  StrCpy $0 $1 1 -1
  ${If} $0 == ";"
    StrCpy $1 "$1$INSTDIR"
  ${Else}
    StrCpy $1 "$1;$INSTDIR"
  ${EndIf}
  WriteRegExpandStr HKCU "${CAI_ENV_KEY}" "Path" $1

  broadcast:
    DetailPrint "Added $INSTDIR to your Path so clipboard-ai works in new terminals."
    !insertmacro CAI_BROADCAST
  end:
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

!macro CAI_DEFINE_REMOVE PREFIX
Function ${PREFIX}CaiRemoveFromPath
  Push $0
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5

  ClearErrors
  ReadRegStr $1 HKCU "${CAI_ENV_KEY}" "Path"
  IfErrors end
  StrCmp $1 "" end
  StrLen $0 $1
  IntOp $0 $0 + 2
  IntCmp $0 ${NSIS_MAX_STRLEN} end 0 end

  StrCpy $2 ";$1;"
  StrCpy $3 ";$INSTDIR;"
  !insertmacro CAI_FIND "${PREFIX}" $2 $3 $0
  ${If} $0 < 0
    StrCpy $3 ";$INSTDIR\;"
    !insertmacro CAI_FIND "${PREFIX}" $2 $3 $0
  ${EndIf}
  IntCmp $0 0 0 end 0

  StrLen $4 $3
  StrCpy $5 $2 $0
  IntOp $4 $0 + $4
  IntOp $4 $4 - 1
  StrCpy $4 $2 "" $4
  StrCpy $2 "$5$4"
  StrCpy $2 $2 "" 1
  StrCpy $2 $2 -1

  ${If} $2 == ""
    DeleteRegValue HKCU "${CAI_ENV_KEY}" "Path"
  ${Else}
    WriteRegExpandStr HKCU "${CAI_ENV_KEY}" "Path" $2
  ${EndIf}
  !insertmacro CAI_BROADCAST

  end:
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
  Pop $0
FunctionEnd
!macroend

!insertmacro CAI_DEFINE_REMOVE "un."

!macro NSIS_HOOK_POSTINSTALL
  Call CaiAddToPath
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Call un.CaiRemoveFromPath
!macroend
