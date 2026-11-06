Get-ChildItem `  
#             ^ no-misleading-backtick
    -Force `
    -Name
$x = 1 + `	
#        ^ no-misleading-backtick
    2
