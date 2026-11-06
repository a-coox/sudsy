Write-Host "hi"
^^^^^^^^^^ no-write-host
write-host
^^^^^^^^^^ no-write-host
Write-Output "ok"
Get-Help Write-Host
iex $script
^^^ no-cmdlet-aliases
^^^ no-invoke-expression
Invoke-Expression -Command $c
^^^^^^^^^^^^^^^^^ no-invoke-expression
ConvertTo-SecureString $p -AsPlainText -Force
^^^^^^^^^^^^^^^^^^^^^^ no-plain-text-secure-string
ConvertTo-SecureString $p
ConvertTo-SecureString `
^^^^^^^^^^^^^^^^^^^^^^ no-plain-text-secure-string
    $p -AsPlainText
Get-WmiObject Win32_Process
^^^^^^^^^^^^^ no-wmi-cmdlets
Get-CimInstance Win32_Process
