.PHONY: all clean
all: build
clean:
build:
	echo
.DEFAULT: ;
empty: all
^^^^^ phony-declared
both one two:
^^^^ phony-declared
#    ^^^ phony-declared
#        ^^^ phony-declared
$(OBJS): ;
