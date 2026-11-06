function Fetch-Data {
#        ^^^^^^^^^^ approved-verbs
    param(
        [string]$Password,
#       ^^^^^^^^^^^^^^^^^ no-plain-text-password
        [Parameter(Mandatory)][string] $UserPassphrase = 'x',
#       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-plain-text-password
        [securestring]$Secret,
        [string]$Name,
        [switch]$Force = $true,
#       ^^^^^^^^^^^^^^^^^^^^^^ no-default-true-switch
        [switch]$Quiet
    )
    try { Get-Item x } catch { }
#                      ^^^^^^^^^ empty-catch-block
    try { Get-Item x } catch {
        # ignored
    }
    try { Get-Item x } catch { Write-Error $_ }
    try { 1 } catch { try { 2 } catch { throw } }
}
function Get-Thing { }
function Fetch { }
function global:Make-Thing { }
#        ^^^^^^^^^^^^^^^^^ approved-verbs
$global:count = 1
^^^^^^^^^^^^^ no-global-vars
$x = $global:count + 1
#    ^^^^^^^^^^^^^ no-global-vars
Set-Item $env:Path -Value $Global:Foo
#                         ^^^^^^^^^^^ no-global-vars
$script:count = 1
$input = 1
^^^^^^^^^^ no-automatic-variable-assignment
$true = 0
^^^^^^^^^ no-automatic-variable-assignment
$count = 1
$x -eq $null
^^^^^^^^^^^^ null-comparison-side
$null -eq $x
if ($a -ne $null -and $b -ceq $null) { }
#   ^^^^^^^^^^^^ null-comparison-side
#                     ^^^^^^^^^^^^^ null-comparison-side
$x -eq $nullable
