//! A tiny shell reader for the text of `RUN`: the simple commands of a script as words, enough for
//! checks on a program and its arguments (not a shell parser: no expansion, no heredocs).

pub struct Cmd {
    pub name: String,
    pub args: Vec<String>,
}

/// Words that start a command line without being the command.
const LEADING: &[&str] = &[
    "if", "then", "else", "elif", "fi", "do", "done", "while", "until", "!", "{", "}", "time",
];

impl Cmd {
    /// The names of the flags: `--long[=v]` is `long`, `-abc` is `a`, `b`, `c` (`--` and `-` are
    /// arguments).
    pub fn flags(&self) -> Vec<String> {
        self.args.iter().flat_map(|a| flag_names(a)).collect()
    }

    pub fn has_flag(&self, name: &str, times: usize) -> bool {
        self.flags().iter().filter(|f| *f == name).count() >= times
    }

    /// The arguments that are not flags, without the value of a flag in `skip` (the next word,
    /// unless the flag has `=value`).
    pub fn positionals(&self, skip: &[String]) -> Vec<&str> {
        let mut out = Vec::new();
        let mut args = self.args.iter();
        while let Some(a) = args.next() {
            let flags = flag_names(a);
            if flags.is_empty() {
                out.push(a.as_str());
            } else if !a.contains('=')
                && (skip.iter().any(|s| s == "*") || flags.iter().any(|f| skip.contains(f)))
            {
                args.next();
            }
        }
        out
    }
}

fn flag_names(arg: &str) -> Vec<String> {
    if arg == "--" || arg == "-" {
        Vec::new()
    } else if let Some(long) = arg.strip_prefix("--") {
        vec![long.split('=').next().unwrap_or("").to_string()]
    } else if let Some(short) = arg.strip_prefix('-') {
        short.chars().map(String::from).collect()
    } else {
        Vec::new()
    }
}

/// The simple commands of `text`, in order. Commands are separated by `;`, `&`, `|`, `(`, `)`,
/// a backtick and a line break; quotes group and are dropped; a redirection and its target are
/// dropped; leading `VAR=value` words and keywords such as `then` are skipped.
pub fn commands(text: &str) -> Vec<Cmd> {
    let text = text.replace("\\\r\n", "").replace("\\\n", "");
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut i = 0;
    let finish_word = |words: &mut Vec<String>, word: &mut String, in_word: &mut bool| {
        if *in_word {
            words.push(std::mem::take(word));
            *in_word = false;
        }
    };
    let finish_cmd = |words: &mut Vec<String>, out: &mut Vec<Cmd>| {
        let mut rest = std::mem::take(words).into_iter().peekable();
        while let Some(w) = rest.peek() {
            let assignment = w.split_once('=').is_some_and(|(n, _)| {
                !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')
            });
            if assignment || LEADING.contains(&w.as_str()) {
                rest.next();
            } else {
                break;
            }
        }
        if let Some(name) = rest.next() {
            out.push(Cmd {
                name,
                args: rest.collect(),
            });
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\'' | '"' => {
                in_word = true;
                i += 1;
                while i < chars.len() && chars[i] != c {
                    if c == '"' && chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                    }
                    word.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                in_word = true;
                i += 1;
                if let Some(&n) = chars.get(i) {
                    word.push(n);
                }
            }
            '#' if !in_word => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            '>' | '<' => {
                // A redirection: `2>&1`, `> file`; its fd and target are not words.
                if word.chars().all(|d| d.is_ascii_digit()) {
                    word.clear();
                    in_word = false;
                }
                i += 1;
                while i < chars.len() && matches!(chars[i], '>' | '<' | '&' | '|') {
                    i += 1;
                }
                while i < chars.len() && chars[i].is_whitespace() && chars[i] != '\n' {
                    i += 1;
                }
                while i < chars.len() && !chars[i].is_whitespace() && !";&|()`".contains(chars[i]) {
                    i += 1;
                }
                continue;
            }
            ';' | '&' | '|' | '(' | ')' | '`' | '\n' => {
                finish_word(&mut words, &mut word, &mut in_word);
                finish_cmd(&mut words, &mut out);
            }
            c if c.is_whitespace() => finish_word(&mut words, &mut word, &mut in_word),
            c => {
                in_word = true;
                word.push(c);
            }
        }
        i += 1;
    }
    finish_word(&mut words, &mut word, &mut in_word);
    finish_cmd(&mut words, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::commands;

    #[test]
    fn splits_commands_and_unquotes() {
        let c = commands("cd /a && apt-get -y install 'x=1' \"y\" 2>&1 | tee f; FOO=1 sudo ls");
        let names: Vec<_> = c.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["cd", "apt-get", "tee", "sudo"]);
        assert_eq!(c[1].args, ["-y", "install", "x=1", "y"]);
        assert!(commands("echo a \\\n  b")[0].args == ["a", "b"]);
    }

    #[test]
    fn flags_and_positionals() {
        let c = &commands("apt-get -qq -t stable install --no-install-recommends a")[0];
        assert!(c.has_flag("q", 2) && c.has_flag("no-install-recommends", 1));
        assert_eq!(c.positionals(&["t".into()]), ["install", "a"]);
    }
}
