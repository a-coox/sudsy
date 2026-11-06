cd build
^^ cd-without-exit
cd .. && make
cd /
cd build || exit
cd build && make
make && cd build
#       ^^ cd-without-exit
if cd build; then make; fi
while cd build; do break; done
cd build | cat
^^ cd-without-exit
pushd build
^^^^^ cd-without-exit
popd -n
popd
^^^^ cd-without-exit
if cd build | cat; then :; fi
{
	cd build
#^^ cd-without-exit
}
f() { cd build; }
#     ^^ cd-without-exit
