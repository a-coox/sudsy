# Runs Invoke-ScriptAnalyzer (the module Save-Module wrote) and prints JSON.
# usage: pwsh -NoProfile -File parity_psscriptanalyzer.ps1 <folder Save-Module wrote> rules
#        pwsh -NoProfile -File parity_psscriptanalyzer.ps1 <folder Save-Module wrote> run <rules,comma separated> <folder of .ps1/.psm1 files>
param([string]$Prefix, [string]$Mode, [string]$Rules, [string]$Folder)
$ErrorActionPreference = 'Stop'
Import-Module (Get-ChildItem "$Prefix/PSScriptAnalyzer/*/PSScriptAnalyzer.psd1").FullName
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
if ($Mode -eq 'rules') {
    Get-ScriptAnalyzerRule | Sort-Object RuleName | ForEach-Object { '{0} {1}' -f $_.RuleName, $_.Severity }
} else {
    $found = Invoke-ScriptAnalyzer -Path $Folder -Recurse -IncludeRule ($Rules -split ',') -ErrorAction SilentlyContinue
    ConvertTo-Json -InputObject @($found | ForEach-Object {
        [pscustomobject]@{ file = $_.ScriptPath; rule = $_.RuleName; line = $_.Line; column = $_.Column; severity = "$($_.Severity)" }
    }) -Compress
}
