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
echo "$(date)s"
read line
^^^^^^^^^ read-without-raw
read -r line
read -p "x" -r line
printf "$fmt"
^^^^^^^^^^^^^ printf-variable-format
printf "%s $x\n" y
printf "a$x" y
^^^^^^^^^^^^^^ printf-variable-format
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
test -f x && echo yes || cleanup
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ and-or-is-not-if-else
test -f x && echo yes || echo no
[ -n x ] && (a || b)
a && b
a || b && c
ls -l | grep txt | wc -l
^^^^^^^^^^^^^^^^^^^^^^^^ grep-count
^^^^^^^^^^^^^^^^^^^^^^^^ no-ls-grep
which git
^^^^^^^^^ no-which
