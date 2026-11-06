if [ $? -eq 0 ]; then :; fi
#  ^^^^^^^^^^^^ test-exit-status
if [ "$?" != 0 ]; then :; fi
#  ^^^^^^^^^^^^^ test-exit-status
[ $x -eq 0 ]
[ -f a -o -f b ]
^^^^^^^^^^^^^^^^ test-and-or
[ -a file ]
[[ -f a && -f b ]]
[ ! -z "$x" ]
^^^^^^^^^^^^^ test-negated-empty
[ ! -n "$x" ]
^^^^^^^^^^^^^ test-negated-empty
[ -z "$x" ]
for f in $(ls); do :; done
^^^^^^^^^^^^^^^^^^^^^^^^^^ iterate-ls-output
for f in `ls /tmp`; do :; done
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ iterate-ls-output
for f in *.txt; do :; done
for f in $(find . -name x); do :; done
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ iterate-find-output
find . | xargs rm
^^^^^^^^^^^^^^^^^ find-xargs-print0
find . -print0 | xargs -0 rm
find . | xargs -0 rm
find . -name x | wc -l
