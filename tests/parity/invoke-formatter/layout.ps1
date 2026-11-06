if ($a)
{
    foo
}
else
{ bar }
if ($a) {
    foo
} elseif ($b) {
    bar
}
try {
    x
} catch [System.Exception] {
    y
} finally { z }
function  Foo
{
    [CmdletBinding()]
    param(
    [string]$a,
        [int]$b=1
    )
    begin { }
    process
    {
    $_
    if ($x) { $y = 1
    }
    }
}
$h = @{
a = 1

    longer=2
# comment
x = @{
k = 1
}
}
$x = 1+2*3
$y=$a-eq$b
$r = 1..3
$z   =   @(1,2 ,3)
foo   -Name  bar -Force
Get-Foo|Bar
Get-Foo |
Bar |
Baz
$a.Foo( 1,2 )
foreach($i in $x){ $i }
while($true){ break }
switch($x){ 1 { 'a' } default { 'b' } }
$s = $a `
    + $b
Write-Host `
  "hi" `
  "there"
$x = foo(@{
  a = 1
})
Foo -Bar @{
a = 1
} -Baz (
1
)
$q = Get-Foo -A 1 |
    Where-Object { $_ } |
    ForEach-Object {
        $_
    }
$list = @(
'a',
        'b'
)
function Bar {
        # indented comment
    Get-Foo   # trailing
        # before close
}
$t = @(1, 2).ForEach{ $_ }
$s = @"
  here $a
"@
