; Only files listed by the installer template are removed. Browser data and
; user-selected runtime paths are intentionally preserved for reinstallation.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; Only remove this installation's optional default-browser registration.
    ReadRegStr $R7 HKCU "Software\Classes\MzHTML\shell\open\command" ""
    ${If} $R7 == "$\"$INSTDIR\${MAINBINARYNAME}.exe$\" -- $\"%1$\""
      DeleteRegKey HKCU "Software\Classes\MzHTML"
      DeleteRegKey HKCU "Software\Clients\StartMenuInternet\MZ指纹浏览器"
      DeleteRegValue HKCU "Software\RegisteredApplications" "MZ指纹浏览器"
    ${EndIf}
  ${EndIf}
!macroend
