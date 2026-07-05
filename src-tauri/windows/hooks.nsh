; TokenTank NSIS installer hooks.
; After a successful interactive install, open the getting-started page.
; Silent installs (/S) skip it — no surprise browser windows in scripted
; or enterprise deployments.

!macro NSIS_HOOK_POSTINSTALL
  IfSilent tokentank_skip_thankyou
  ExecShell "open" "https://agentshortlist.com/tokentank/thank-you-for-installing"
  tokentank_skip_thankyou:
!macroend
