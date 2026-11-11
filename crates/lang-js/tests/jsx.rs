//! JSX against Prettier's output (each expectation was checked with Prettier 3 under its
//! defaults), with every check of the shared helper.

use core_format::testing::assert_formats;
use lang_js::JsProvider;
use provider_api::format_testing::Formatting;

fn fmt(input: &str) -> String {
    assert_formats(&Formatting::new(JsProvider::new(), "a.jsx"), input)
}

#[test]
fn elements_text_and_whitespace() {
    assert_eq!(
        fmt(
            r#"const a = <div className='x'>hello <b>world</b> and some more text that goes on and on and on and on for a long while ok</div>;
function App() {
  return <div><Foo bar="1" baz={2} /><span>text</span>{cond && <p>yes</p>}{" "}
  {list.map(i => <li key={i}>{i}</li>)}</div>;
}
const x = <></>;
const y = <Foo {...props} a="it's" b='say "hi"' c={`t`}>  </Foo>;
"#
        ),
        r#"const a = (
  <div className="x">
    hello <b>world</b> and some more text that goes on and on and on and on for
    a long while ok
  </div>
);
function App() {
  return (
    <div>
      <Foo bar="1" baz={2} />
      <span>text</span>
      {cond && <p>yes</p>}{" "}
      {list.map((i) => (
        <li key={i}>{i}</li>
      ))}
    </div>
  );
}
const x = <></>;
const y = (
  <Foo {...props} a="it's" b='say "hi"' c={`t`}>
    {" "}
  </Foo>
);
"#
    );
}

#[test]
fn conditions_blank_attributes_and_quotes() {
    assert_eq!(
        fmt(r#"<div>{a ? "a" : b ? "b" : "c"}</div>;
normalModeNonBreaking ? "a" : "b";
cable ? "satellite" : isPublic ? "affairs" : network ? <span id="c" /> : "dun";
const basic = (
  <Component
    first={1}

    second={2}
  />
);
const el = (
  <div>
    <div>First</div>
    Second
  </div>
);
<div id="&quot;'<>&amp;quot;" />;
<div id='"&#39;<>&amp;quot;' />;
<div id='123' />;
<div id='&#39;&quot;' />;
{a && b && <span>make this text just so long enough to break this to the next line</span>}
"#),
        r#"<div>{a ? "a" : b ? "b" : "c"}</div>;
normalModeNonBreaking ? "a" : "b";
cable ? "satellite" : isPublic ? "affairs" : network ? <span id="c" /> : "dun";
const basic = (
  <Component
    first={1}

    second={2}
  />
);
const el = (
  <div>
    <div>First</div>
    Second
  </div>
);
<div id="&quot;'<>&amp;quot;" />;
<div id='"&#39;<>&amp;quot;' />;
<div id="123" />;
<div id='&#39;"' />;
{
  a && b && (
    <span>
      make this text just so long enough to break this to the next line
    </span>
  );
}
"#
    );
}

#[test]
fn text_beside_expressions() {
    assert_eq!(
        fmt(r#"a = <div><br />
  text text text text text text text text text text text {this.props.type}</div>
b = <div><br />
  text text text text text text text text text text text {this.props.type} more</div>
c = <div><br />
  text text text text text text text text text text text {this.props.type} </div>
"#),
        r#"a = (
  <div>
    <br />
    text text text text text text text text text text text {this.props.type}
  </div>
);
b = (
  <div>
    <br />
    text text text text text text text text text text text {
      this.props.type
    }{" "}
    more
  </div>
);
c = (
  <div>
    <br />
    text text text text text text text text text text text{" "}
    {this.props.type}{" "}
  </div>
);
"#
    );
}

#[test]
fn prettier_ignore_and_templates() {
    assert_eq!(
        fmt(r#"<style jsx>{`
  div {
    color: red;
  }
`}</style>;
<div>
  {/* prettier-ignore */}
  <span     ugly  format=''   />
  <b   a />
</div>;
"#),
        r#"<style jsx>{`
  div {
    color: red;
  }
`}</style>;
<div>
  {/* prettier-ignore */}
  <span     ugly  format=''   />
  <b a />
</div>;
"#
    );
}

#[test]
fn jsx_single_quote_and_bracket_same_line() {
    use core_config::OptionValue::Bool;
    let mut subject = Formatting::new(JsProvider::new(), "a.jsx");
    subject.options = vec![
        ("jsx_single_quote", Bool(true)),
        ("bracket_same_line", Bool(true)),
    ];
    assert_eq!(
        assert_formats(
            &subject,
            "x = <Foo aaaaaaaaaaaaaaaaaaaaa=\"1\" bbbbbbbbbbbbbbbbbbbbbbbbb='it\"s' cccccccccccccccccccccc=\"d\">t</Foo>;\n"
        ),
        "x = (\n  <Foo\n    aaaaaaaaaaaaaaaaaaaaa='1'\n    bbbbbbbbbbbbbbbbbbbbbbbbb='it\"s'\n    cccccccccccccccccccccc='d'>\n    t\n  </Foo>\n);\n"
    );
}
