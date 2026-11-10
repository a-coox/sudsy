use provider_api::format_testing::{assert_file_names, assert_formats, check_corpus};

/// Formats with every check of the shared helper (round trip, meaning, idempotency, comments).
#[track_caller]
fn fmt(input: &str) -> String {
    assert_formats(lang_makefile::provider(), "Makefile", input)
}

/// Every Makefile of the pinned corpus (`tests/conformance/fetch-makefile-corpus.sh`) parses,
/// formats with the same tokens (the safety check) and idempotently.
///
/// `cargo test --profile corpus -p lang-makefile --test makefile -- --ignored --nocapture conformance`
#[test]
#[ignore]
fn conformance() {
    let (count, unparsed) = check_corpus(lang_makefile::provider(), "Makefile", "makefile", &[]);
    println!("{count} files, {} with syntax errors", unparsed.len());
    assert!(
        unparsed.is_empty(),
        "valid files with syntax errors: {unparsed:#?}"
    );
}

#[test]
fn grammar_fork_constructs() {
    let src = "ifeq (a,b)\n  X = $(dir)\nelse\n  override Y = $(if $(a),b,)\nendif\nS = ( \\\n\tls \\\n\t)\nall:\n\t@for m in $$($(GO) list a); do x; done\n\t$(info goal(s) $(@:%.o=%.c))\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn assignments_and_rules() {
    assert_eq!(
        fmt(
            "CC   :=  gcc   \nCFLAGS=-O2 -Wall \\\n\t-g\nexport FOO?=bar\n\n\n\n\nall : main.o   util.o|outdir  # c\n\t$(CC) -o $@ $^   \n\t@echo \"done\"; \\\n\t  ls\n\n%.o:%.c\n\t  -$(CC) -c $< -o $@\n"
        ),
        "CC := gcc\nCFLAGS = -O2 -Wall \\\n\t-g\nexport FOO?=bar\n\n\nall: main.o util.o | outdir # c\n\t$(CC) -o $@ $^\n\t@echo \"done\"; \\\n\t  ls\n\n%.o: %.c\n\t-$(CC) -c $< -o $@\n"
    );
}

#[test]
fn conditionals_and_define() {
    assert_eq!(
        fmt(
            "ifeq ($(OS),Windows_NT)\nEXE:=.exe\nelse ifdef X\n  EXE :=\nelse\nEXE:=.bin\nendif\ndefine RULE\n$(1): ; echo $(1)\nendef\n$(eval $(call RULE,a))\nclean:\nifdef V\n\techo verbose\n  endif\n\trm -f *.o\n"
        ),
        "ifeq ($(OS),Windows_NT)\n  EXE := .exe\nelse ifdef X\n  EXE :=\nelse\n  EXE := .bin\nendif\ndefine RULE\n$(1): ; echo $(1)\nendef\n$(eval $(call RULE,a))\nclean:\nifdef V\n\techo verbose\nendif\n\trm -f *.o\n"
    );
}

#[test]
fn define_bodies_are_kept() {
    let src = "define X\n\ta  \n  b\n\nc\nendef\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn a_trailing_continuation_keeps_the_next_line_out_of_the_value() {
    let src = "A= \\\n\t-Wa \\\n\n\nB= x\n";
    assert_eq!(fmt(src), "A = \\\n\t-Wa \\\n\n\nB = x\n");
}

#[test]
fn special_rules() {
    assert_eq!(
        fmt(
            "foo: CFLAGS+=-g\n$(OBJS): %.o: %.c ; $(CC) -c $<\na b &: c\nd::e\n\techo $$HOME ; true\nvpath %.c src\n"
        ),
        "foo: CFLAGS+=-g\n$(OBJS): %.o: %.c ; $(CC) -c $<\na b &: c\nd:: e\n\techo $$HOME ; true\nvpath %.c src\n"
    );
}

#[test]
fn comments_are_kept() {
    let src = "# top\nA = 1 # t\n\n# b\nall:\n\techo 1\n# between\n\techo 2\n# tail\n";
    assert_eq!(fmt(src), src);
}

#[test]
fn matches_the_file_name() {
    assert_file_names(
        &lang_makefile::provider(),
        &["Makefile", "a/GNUmakefile", "makefile", "x.mk", "x.mak"],
        &["Makefile.in", "a.make", "mk"],
    );
}
