rm $(cat list)
#  ^^^^^^^^^^^ quote-command-substitution
rm "$(cat list)"
rm a$(b) `c`
#   ^^^^ quote-command-substitution
#        ^^^ no-backticks
#        ^^^ quote-command-substitution
rm '$(literal)' "$(ok)"
echo $((1 + 2))
rm $file ${dir}/x "$file" '$file' $1 $?
#  ^^^^^ quote-variable
#        ^^^^^^^^ quote-variable
#                                 ^^ quote-variable
rm --opt=$value "x $y"
#  ^^^^^^^^^^^^ quote-variable
cp $@ dir
#  ^^ quote-at-expansion
cp "$@" dir "${arr[@]}" ${arr[@]}
#                       ^^^^^^^^^ quote-at-expansion
cp $* dir ${arr[*]}
#  ^^ quote-star-expansion
#  ^^ quote-variable
#         ^^^^^^^^^ quote-star-expansion
#         ^^^^^^^^^ quote-variable
now=`date`
#   ^^^^^^ no-backticks
echo `date` "`date`" 'a`b`'
#    ^^^^^^ no-backticks
#    ^^^^^^ quote-command-substitution
#            ^^^^^^ no-backticks
[ -n `foo` ]
#    ^^^^^ no-backticks
#    ^^^^^ quote-command-substitution
[ $x -eq 0 ]
# ^^ quote-variable
[[ $x = $y ]] && [[ -n $(foo) ]]
export out=$(make)
^^^^^^^^^^^^^^^^^^ declare-and-assign-separately
local out="$(make)" x=1
^^^^^^^^^^^^^^^^^^^^^^^ declare-and-assign-separately
declare plain=1
for f in $files; do :; done
