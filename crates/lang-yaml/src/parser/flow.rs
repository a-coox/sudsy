//! Flow nodes: properties, aliases, scalars and flow collections.

use super::scan::is_flow_indicator;
use super::{Parser, R};
use crate::generated::kind as k;

/// What `flow_node_cand` found: a node that may turn out to be an implicit key.
#[derive(Default)]
pub(crate) struct Cand {
    /// A `PropertyNode` was started and is not finished: its content may follow on later lines.
    pub open_prop: bool,
    pub has_content: bool,
    /// Quoted scalar or flow collection: its `:` may be followed directly by the value.
    pub json_like: bool,
    pub multiline: bool,
    /// Properties followed by `|` or `>`.
    pub block_scalar_next: bool,
    /// Properties followed by the end of the line.
    pub eol_after_props: bool,
    /// Bit 0: a tag, bit 1: an anchor.
    pub prop_mask: u8,
}

impl Parser<'_> {
    /// Properties and content of a node in block context (its first line), with continuation
    /// lines of flow scalars indented more than `n`.
    pub(crate) fn flow_node_cand(&mut self, n: i64) -> R<Cand> {
        let saved = self.min_col;
        self.min_col = (n + 1).max(0) as usize;
        let r = self.node_cand();
        self.min_col = saved;
        r
    }

    /// Properties and content of a node.
    pub(crate) fn node_cand(&mut self) -> R<Cand> {
        let mut c = Cand::default();
        if matches!(self.cur(), b'!' | b'&') {
            self.start(k::PROPERTY_NODE);
            c.open_prop = true;
            c.prop_mask = self.properties()?;
            if self.in_flow == 0 {
                if !self.at_content() {
                    c.eol_after_props = true;
                    return Ok(c);
                }
                if matches!(self.cur(), b'|' | b'>') {
                    c.block_scalar_next = true;
                    return Ok(c);
                }
            }
        }
        match self.cur() {
            b'*' => {
                if c.open_prop {
                    return self.fail("alias-with-properties", "an alias cannot have properties");
                }
                self.start(k::ALIAS_NODE);
                self.scan_alias()?;
                self.finish();
                c.has_content = true;
            }
            b'"' => {
                self.start(k::SCALAR);
                c.multiline = self.scan_double()?;
                self.finish();
                c.has_content = true;
                c.json_like = true;
            }
            b'\'' => {
                self.start(k::SCALAR);
                c.multiline = self.scan_single()?;
                self.finish();
                c.has_content = true;
                c.json_like = true;
            }
            b'[' | b'{' => {
                c.multiline = self.flow_collection()?;
                c.has_content = true;
                c.json_like = true;
            }
            b'|' | b'>' if self.in_flow > 0 => {
                return self.fail(
                    "block-scalar-in-flow",
                    "a block scalar inside a flow collection",
                );
            }
            _ if self.plain_start() => {
                self.start(k::SCALAR);
                c.multiline = self.scan_plain();
                self.finish();
                c.has_content = true;
            }
            _ => {}
        }
        if c.has_content && c.open_prop {
            self.finish();
            c.open_prop = false;
        }
        Ok(c)
    }

    /// Parses `[...]` or `{...}`; returns whether it spans lines.
    fn flow_collection(&mut self) -> R<bool> {
        let open = self.cur();
        let is_seq = open == b'[';
        let close = if is_seq { b']' } else { b'}' };
        let first_line = self.line_start(self.pos);
        self.start(if is_seq {
            k::FLOW_SEQUENCE
        } else {
            k::FLOW_MAPPING
        });
        self.tok(
            if is_seq { k::L_BRACKET } else { k::L_BRACE },
            self.pos,
            self.pos + 1,
        );
        self.in_flow += 1;
        loop {
            self.skip_flow_ws()?;
            let c = self.cur();
            if self.eof() {
                return self.fail("unterminated-flow", "unterminated flow collection");
            }
            if c == close {
                let p = self.pos;
                self.tok(if is_seq { k::R_BRACKET } else { k::R_BRACE }, p, p + 1);
                break;
            }
            if c == b',' || c == b']' || c == b'}' {
                return self.fail("unexpected-indicator", "unexpected flow indicator");
            }
            self.flow_entry(is_seq)?;
            self.skip_flow_ws()?;
            let c = self.cur();
            if c == b',' {
                let p = self.pos;
                self.tok(k::COMMA, p, p + 1);
            } else if c != close {
                return self.fail("expected-comma", "expected `,` or the closing bracket");
            }
        }
        self.in_flow -= 1;
        self.finish();
        Ok(self.line_start(self.pos) != first_line)
    }

    /// Can a `:` at the current position be a mapping value indicator in flow context?
    fn flow_colon(&self, json_like: bool) -> bool {
        self.cur() == b':'
            && (json_like || self.is_bb(self.pos + 1) || is_flow_indicator(self.at(self.pos + 1)))
    }

    fn at_flow_end(&self) -> bool {
        matches!(self.cur(), b',' | b']' | b'}')
    }

    /// An entry of a flow sequence (a node or a single pair) or flow mapping (always a pair).
    fn flow_entry(&mut self, is_seq: bool) -> R<()> {
        let cp = self.events.len();
        if self.cur() == b'?'
            && (self.is_bb(self.pos + 1) || is_flow_indicator(self.at(self.pos + 1)))
        {
            self.start(k::FLOW_PAIR);
            let p = self.pos;
            self.tok(k::QUESTION, p, p + 1);
            self.skip_flow_ws()?;
            if !self.flow_colon(false) && !self.at_flow_end() {
                self.flow_node()?;
            }
            self.skip_flow_ws()?;
            if self.flow_colon(false) {
                self.flow_value()?;
            }
            self.finish();
            return Ok(());
        }
        let cand = self.node_cand()?;
        let after_node = self.pos;
        if is_seq {
            self.skip_inline_ws();
        } else {
            self.skip_flow_ws()?;
        }
        if self.flow_colon(cand.json_like) {
            if cand.open_prop {
                self.finish();
            }
            if cand.multiline && is_seq {
                return self.fail("multiline-key", "an implicit key must be on one line");
            }
            self.wrap_from(cp, &[k::FLOW_PAIR]);
            self.flow_value()?;
            self.finish();
            return Ok(());
        }
        self.pos = after_node;
        if !cand.has_content && !cand.open_prop {
            return self.fail(
                "unexpected-character",
                "unexpected character in a flow collection",
            );
        }
        if cand.open_prop {
            self.finish();
        }
        if !is_seq {
            self.wrap_from(cp, &[k::FLOW_PAIR]);
            self.finish();
        }
        Ok(())
    }

    /// `:` and the value that follows it, if any.
    fn flow_value(&mut self) -> R<()> {
        let p = self.pos;
        self.tok(k::COLON, p, p + 1);
        self.skip_flow_ws()?;
        if !self.at_flow_end() {
            self.flow_node()?;
        }
        Ok(())
    }

    /// A complete node inside a flow collection; properties without content are an empty node.
    fn flow_node(&mut self) -> R<()> {
        let cand = self.node_cand()?;
        if cand.open_prop {
            self.finish();
        } else if !cand.has_content {
            return self.fail(
                "unexpected-character",
                "unexpected character in a flow collection",
            );
        }
        Ok(())
    }
}
