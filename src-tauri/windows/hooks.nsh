; Regra do Firewall do Windows para a transmissão por link: libera só a entrada TCP do
; executável do CapturaX. O instalador roda como administrador (installMode perMachine).

!macro NSIS_HOOK_POSTINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="CapturaX Stream"'
  nsExec::Exec 'netsh advfirewall firewall add rule name="CapturaX Stream" dir=in action=allow program="$INSTDIR\${MAINBINARYNAME}.exe" protocol=TCP enable=yes profile=any'
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'netsh advfirewall firewall delete rule name="CapturaX Stream"'
!macroend
