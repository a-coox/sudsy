A = $(shell date +%s)
^ timestamp-expanded
B := $(shell date +%s)
C ?= a date
^ timestamp-expanded
D = date
E += $(shell date)
^ timestamp-expanded
F ::= $(shell date)
G != date
foo: H = $(shell date)
^^^ timestamp-expanded
