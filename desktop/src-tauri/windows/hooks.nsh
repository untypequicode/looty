; Crochets de l'installateur NSIS de Looty (installation « pour l'utilisateur », sans droits
; administrateur) : lien looty://, identité des notifications, nettoyage à la désinstallation.

!macro NSIS_HOOK_POSTINSTALL
  ; Liens looty://… : le système lance « looty.exe "looty://…" » (instance déjà ouverte comprise).
  WriteRegStr HKCU "Software\Classes\looty" "" "URL:Looty"
  WriteRegStr HKCU "Software\Classes\looty" "URL Protocol" ""
  WriteRegStr HKCU "Software\Classes\looty\DefaultIcon" "" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr HKCU "Software\Classes\looty\shell\open\command" "" '"$INSTDIR\${MAINBINARYNAME}.exe" "%1"'
  ; Identité de l'application pour les notifications Windows (toasts).
  WriteRegStr HKCU "Software\Classes\AppUserModelId\app.looty.desktop" "DisplayName" "Looty"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DeleteRegKey HKCU "Software\Classes\looty"
  DeleteRegKey HKCU "Software\Classes\AppUserModelId\app.looty.desktop"
  ; Lancement à l'ouverture de session, s'il était activé.
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Looty"
!macroend
