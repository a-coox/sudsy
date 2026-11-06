gci -Recurse | % { $_ }
^^^ no-cmdlet-aliases
#              ^ no-cmdlet-aliases
Get-ChildItem | ? Name -eq x
#               ^ no-cmdlet-aliases
ls
^^ no-cmdlet-aliases
& ls
Get-ChildItem | Where-Object { $_ } | foreach { $_ }
#                                     ^^^^^^^ no-cmdlet-aliases
$x = dir
#    ^^^ no-cmdlet-aliases
Write-Output ls
