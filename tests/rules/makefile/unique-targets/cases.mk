a:
	echo
a:
^ unique-targets
	echo
b::
b::
.PHONY: a
.PHONY: b
c d:
^ phony-declared
# ^ phony-declared
d:
^ phony-declared
^ unique-targets
