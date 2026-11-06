cat in.txt | grep x
^^^^^^^^^^^^^^^^^^^ useless-cat
cat -n in.txt | grep x
cat a b | grep x
grep x in.txt
echo $(date)
^^^^^^^^^^^^ useless-echo
#    ^^^^^^^ quote-command-substitution
echo "$(date)"
^^^^^^^^^^^^^^ useless-echo
echo `date`
^^^^^^^^^^^ useless-echo
#    ^^^^^^ quote-command-substitution
echo "$(date) now" $x
read line
^^^^^^^^^ read-without-raw
read -r line
read -p "x" -r line
printf "$fmt"
^^^^^^^^^^^^^ printf-variable-format
printf "%s $x\n" y
^^^^^^^^^^^^^^^^^^ printf-variable-format
printf '%s' "$x" "a $b"
printf 'literal $x'
trap "rm $tmp" EXIT
^^^^^^^^^^^^^^^^^^^ trap-expands-now
trap 'rm $tmp' EXIT
trap "echo bye" EXIT
ls | grep txt
^^^^^^^^^^^^^ no-ls-grep
ps aux | grep nginx
^^^^^^^^^^^^^^^^^^^ no-ps-grep
grep x f | wc -l
^^^^^^^^^^^^^^^^ grep-count
egrep x f
^^^^^^^^^ no-egrep
fgrep x f
^^^^^^^^^ no-egrep
which git
^^^^^^^^^ no-which
test -f x && echo yes || echo no
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ and-or-is-not-if-else
a && b
a || b && c
ls -l | grep txt | wc -l
^^^^^^^^^^^^^^^^^^^^^^^^ no-ls-grep
which git
^^^^^^^^^ no-which
