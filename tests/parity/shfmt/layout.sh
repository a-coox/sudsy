#!/bin/bash

# blanks, semicolons, redirects
set -eu


echo  a ;  echo b;
cmd 2> /dev/null >out
cat <<EOF | grep x
  body   $HOME
EOF

if [ -f a ]
then
  echo x
  # inner
elif b; then y
else
    z
fi
if a; then b; fi

for f in a b \
   c
do
  echo "$f"
done
while read -r l; do echo "$l"; done < file

case "$1" in
  a|b)
    echo one
    ;;
  c) echo two ;;
  *)
    ;;
esac

foo()
{
    { a; b; }
    (cd x && make)
}
function bar { :; }

a &&
   b ||
  c
x |
 y
! cmd
