# Prints the file formatted by Invoke-Formatter with the CodeFormatting settings minus PSUseCorrectCasing
# (it needs the command tables of the PowerShell that runs it). Two things of file handling, not
# layout, follow Sudsy: a byte order mark stays, and the output has no blank lines at its start and
# ends with one line break.
# usage: pwsh -NoProfile -File parity_invoke_formatter.ps1 <folder Save-Module wrote> <file>
param([string]$Prefix, [string]$Path)
$ErrorActionPreference = 'Stop'
$module = (Get-ChildItem "$Prefix/PSScriptAnalyzer/*/PSScriptAnalyzer.psd1").FullName
Import-Module $module
$settings = Import-PowerShellDataFile (Join-Path (Split-Path $module) 'Settings/CodeFormatting.psd1')
$settings.IncludeRules = [string[]]@($settings.IncludeRules | Where-Object { $_ -ne 'PSUseCorrectCasing' })
$settings.Rules.Remove('PSUseCorrectCasing')
$bytes = [System.IO.File]::ReadAllBytes($Path)
$bom = $bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF
$out = Invoke-Formatter -ScriptDefinition ([System.IO.File]::ReadAllText($Path)) -Settings $settings
$eol = if ($out.Contains("`r`n")) { "`r`n" } else { "`n" }
$out = $out.Trim("`r", "`n")
if ($out.Length -gt 0) { $out += $eol }
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
[Console]::Out.Write(($(if ($bom) { [string][char]0xFEFF } else { '' }) + $out))
