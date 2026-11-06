//! Markdown container/link scanning for `link_span` (MEL-02).
//!
//! Container model (CommonMark-faithful subset, no external crates):
//! 1. A frontmatter block (`---` … `---`) is excluded until the YAML host
//!    (Task 05) owns its scalars — an unsupported YAML shape can never leak
//!    `[[links]]` into the body scan by accident.
//! 2. Outside excluded zones we walk bytes recognizing: fenced code
//!    (``` / ~~~ with matching length), indented code (4+ spaces after a
//!    blank line), inline code (backtick runs paired by length), `%%…%%`
//!    comments, `<!-- … -->` comments, wiki links / embeds, and Markdown
//!    links with escape and nested-paren awareness.
//! 3. Ranges are validated with `str::get` before use; all pushes happen
//!    on char boundaries.

use super::html;
use super::math;
use super::yaml;
use super::{DocumentScan, Host, LinkToken, ScanReason, Syntax};

/// Upper bound on how far one failed construct search may scan past its
/// opening bytes. Crafted documents (`[`/`[[`/`<!--` floods) make failed
/// searches common; without a bound each failure rescans a huge span and
/// the whole scan degrades to quadratic.
const MAX_CONSTRUCT_RESCAN: usize = 64 * 1024;

/// Total construct-search work budget before the scan degrades. Debt is
/// charged per byte searched by the non-telescoping helpers (wiki close,
/// bracket/destination matching, comment closes). Once debt exceeds this
/// budget, those construct attempts stop — the walker keeps advancing
/// monotonically, so total scan work stays O(document) even for hostile
/// inputs; links past that point in a hostile file are under-reported
/// (reported via `ScanReason::SearchBudget`, same honesty class as an
/// unclosed host swallowing the rest of the file).
fn search_debt_cap(source_len: usize) -> usize {
    2 * source_len + 1024 * 1024
}

pub(super) fn scan(source: &str, max_links: usize) -> DocumentScan {
    let mut scanner = Scanner {
        source,
        bytes: source.as_bytes(),
        excluded: Vec::new(),
        tokens: Vec::new(),
        reasons: Vec::new(),
        max_links,
        link_capped: false,
        skip_to: 0,
        previous_blank: true,
        search_debt: 0,
        degraded: false,
    };
    scanner.scan_blocks();
    scanner.scan_inline();
    let mut reasons = scanner.reasons;
    if scanner.link_capped {
        reasons.push(ScanReason::LinkCap);
    }
    if scanner.degraded {
        reasons.push(ScanReason::SearchBudget);
    }
    // Kept sorted+coalesced by push_excluded; sort again is a no-op safety.
    scanner.excluded.sort_by_key(|r| r.start);
    DocumentScan {
        tokens: scanner.tokens,
        excluded: scanner.excluded,
        reasons,
    }
}

pub(super) fn percent_decode(raw: &str) -> anyhow::Result<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| b.is_ascii_hexdigit();
            if hex(bytes[i + 1]) && hex(bytes[i + 2]) {
                let value = u8::from_str_radix(&raw[i + 1..i + 3], 16)
                    .map_err(|_| anyhow::anyhow!("invalid_path: bad percent escape"))?;
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| anyhow::anyhow!("invalid_path: path is not UTF-8"))
}

struct Scanner<'a> {
    source: &'a str,
    bytes: &'a [u8],
    excluded: Vec<std::ops::Range<usize>>,
    tokens: Vec<LinkToken>,
    reasons: Vec<ScanReason>,
    max_links: usize,
    link_capped: bool,
    /// Block-phase cursor: inline scanning and later block lines resume
    /// from here after a multi-line host was consumed.
    skip_to: usize,
    previous_blank: bool,
    /// Construct-search debt (see `search_debt_cap`).
    search_debt: usize,
    /// Set when the debt cap tripped; surfaces as `SearchBudget`.
    degraded: bool,
}

impl<'a> Scanner<'a> {
    fn scan_blocks(&mut self) {
        // Frontmatter: the YAML host (MEL-05) owns the block; wiki links
        // inside supported scalar shapes become Yaml-hosted tokens, and an
        // unsupported shape marks the document incomplete.
        if self.source.starts_with("---\n") || self.source.starts_with("---\r\n") {
            if let Some(close) = self.find_frontmatter_close() {
                let yaml_scan = yaml::scan_frontmatter(self.source, 0..close);
                if yaml_scan.unsupported {
                    self.reasons.push(ScanReason::UnsupportedHost);
                }
                for token in yaml_scan.tokens {
                    self.push_token(token);
                }
                self.push_excluded(0..close);
                self.skip_to = close;
            }
        }

        let mut offset = self.skip_to;
        while offset < self.source.len() {
            let line_end = self.source[offset..]
                .find('\n')
                .map(|rel| offset + rel + 1)
                .unwrap_or(self.source.len());
            let line = &self.source[offset..line_end];
            let trimmed = line.trim_end_matches(['\r', '\n']);
            let indent = line.len() - line.trim_start_matches([' ', '\t']).len();

            let mut consumed_to = line_end;

            // Fences need indent <= 3; the other hosts apply regardless of
            // indent once we are not already inside one of their blocks.
            if let Some((fence_char, fence_len)) = fence_marker(trimmed) {
                if indent <= 3 {
                    match self.find_fence_close(line_end, fence_char, fence_len) {
                        Some(end) => {
                            self.push_excluded(offset..end);
                            consumed_to = end;
                        }
                        None => {
                            // Unclosed fence owns the rest of the document.
                            self.push_excluded(offset..self.source.len());
                            consumed_to = self.source.len();
                        }
                    }
                }
            } else if trimmed.starts_with("%%") && !trimmed.ends_with("%%") && trimmed.len() > 2 {
                let tail = &self.source[(offset + 2).min(self.source.len())..];
                let window = &tail[..tail.len().min(MAX_CONSTRUCT_RESCAN)];
                if let Some(rel) = window.find("%%") {
                    let end = offset + 2 + rel + 2;
                    self.push_excluded(offset..end);
                    consumed_to = end;
                }
            } else if trimmed.starts_with("<!--") && !trimmed.contains("-->") {
                let tail = &self.source[offset..];
                let window = &tail[..tail.len().min(MAX_CONSTRUCT_RESCAN)];
                if let Some(rel) = window.find("-->") {
                    let end = offset + rel + 3;
                    self.push_excluded(offset..end);
                    consumed_to = end;
                }
            } else if trimmed.starts_with("%%") && trimmed.ends_with("%%") && trimmed.len() > 4 {
                // Single-line comment opens and closes on this line: the
                // whole line is comment (R13 regression — the old inline
                // pass re-paired it, which the debt probes now skip).
                self.push_excluded(offset..line_end);
                consumed_to = line_end;
            } else if trimmed.starts_with("<!--") && trimmed.contains("-->") {
                // Single-line comment: exclude only through the close
                // marker — a live link may follow on the same line.
                let tail = &self.source[offset..];
                let window = &tail[..tail.len().min(MAX_CONSTRUCT_RESCAN)];
                if let Some(rel) = window.find("-->") {
                    let end = offset + rel + 3;
                    self.push_excluded(offset..end);
                    consumed_to = end;
                }
            } else if indent >= 4 && self.previous_blank && !trimmed.is_empty() {
                let end = self.find_indented_code_end(offset);
                self.push_excluded(offset..end);
                consumed_to = end;
            }

            self.previous_blank = trimmed.is_empty();
            offset = consumed_to.max(line_end);
            self.skip_to = offset;
        }
    }

    fn find_frontmatter_close(&self) -> Option<usize> {
        let mut offset = 0;
        for line in self.source.split_inclusive('\n') {
            if offset > 0 && line.trim_end() == "---" {
                return Some(offset + line.len());
            }
            offset += line.len();
        }
        None
    }

    fn find_fence_close(&self, from: usize, fence_char: u8, fence_len: usize) -> Option<usize> {
        let mut offset = from;
        for line in self.source[from..].split_inclusive('\n') {
            let trimmed = line.trim();
            if trimmed.len() >= fence_len
                && trimmed.bytes().take(fence_len).all(|b| b == fence_char)
            {
                return Some(offset + line.len());
            }
            offset += line.len();
        }
        None
    }

    fn find_indented_code_end(&self, from: usize) -> usize {
        let mut offset = from;
        let mut end = from;
        for line in self.source[from..].split_inclusive('\n') {
            let trimmed_start = line.trim_start_matches([' ', '\t']);
            let indent = line.len() - trimmed_start.len();
            if trimmed_start.trim().is_empty() {
                // Blank line inside the block: tentatively included.
                end = offset + line.len();
                offset = end;
                continue;
            }
            if indent >= 4 {
                end = offset + line.len();
                offset = end;
            } else {
                break;
            }
        }
        end
    }

    fn scan_inline(&mut self) {
        let mut i = 0;
        while i < self.bytes.len() {
            if !self.source.is_char_boundary(i) {
                i += 1;
                continue;
            }
            if self.in_excluded(i, i + 1) {
                i += 1;
                continue;
            }
            match self.bytes[i] {
                b'$' => {
                    // Math host (MEL-R2-02): $…$ / $$…$$ own their content.
                    if !escaped_at(self.bytes, i) {
                        match math::math_host_end(self.source, i) {
                            Some(end) => {
                                self.push_excluded(i..end);
                                i = end;
                            }
                            None => {
                                let run =
                                    self.bytes[i..].iter().take_while(|&&b| b == b'$').count();
                                if run >= 2 {
                                    // Unclosed block math: the host boundary
                                    // is unprovable — document not complete.
                                    self.reasons.push(ScanReason::UnsupportedHost);
                                    self.push_excluded(i..self.bytes.len());
                                    i = self.bytes.len();
                                } else {
                                    // Unpaired single $: back to the body.
                                    i += 1;
                                }
                            }
                        }
                    } else {
                        i += 1;
                    }
                }
                b'`' => {
                    let run = backtick_run(self.bytes, i);
                    match find_backtick_close(self.bytes, i + run, run) {
                        Some(end) => {
                            self.push_excluded(i..end);
                            i = end;
                        }
                        None => {
                            // Failed pair search: charge what it scanned
                            // (to EOF) so hostile run-length patterns stay
                            // bounded overall.
                            let scanned = self.bytes.len().saturating_sub(i + run);
                            if !self.charge_debt(scanned) {
                                // Budget tripped: exclude the rest rather
                                // than keep attempting — walk stays linear.
                                self.push_excluded(i..self.bytes.len());
                                break;
                            }
                            i += run;
                        }
                    }
                }
                b'%' if self.bytes.get(i + 1) == Some(&b'%') => {
                    if !self.charge_debt(0) {
                        i += 2;
                        continue;
                    }
                    let tail = &self.source[(i + 2).min(self.source.len())..];
                    let window = &tail[..tail.len().min(MAX_CONSTRUCT_RESCAN)];
                    if let Some(rel) = window.find("%%") {
                        let end = i + 2 + rel + 2;
                        self.push_excluded(i..end);
                        i = end;
                    } else {
                        i += 2;
                    }
                }
                b'<' if self.source[i..].starts_with("<!--") => {
                    let tail = &self.source[i.min(self.source.len())..];
                    let window = &tail[..tail.len().min(MAX_CONSTRUCT_RESCAN)];
                    if let Some(rel) = window.find("-->") {
                        let end = i + rel + 3;
                        self.push_excluded(i..end);
                        i = end;
                    } else {
                        i += 4;
                    }
                }
                b'<' => {
                    // HTML host (MEL-R2-03): the element owns its tag bytes
                    // and, for non-void elements, its content until the
                    // matching close tag. Raw-text elements skip content
                    // parsing entirely.
                    match html::open_tag(self.source, i) {
                        Some((open_end, name, void)) => {
                            if void {
                                self.push_excluded(i..open_end);
                                i = open_end;
                            } else {
                                let search_from = if html::is_raw_text(&name) {
                                    open_end
                                } else {
                                    // For normal elements the close search
                                    // must not stop inside a NESTED same-name
                                    // child; find_close_tag handles nesting
                                    // by scanning for the first matching
                                    // close, which is correct for our
                                    // subset (nested same-name elements
                                    // collapse into the outer range).
                                    open_end
                                };
                                match html::find_close_tag(self.source, search_from, &name) {
                                    Some(end) => {
                                        self.push_excluded(i..end);
                                        i = end;
                                    }
                                    None => {
                                        // Unclosed container: boundary
                                        // unprovable.
                                        self.reasons.push(ScanReason::UnsupportedHost);
                                        self.push_excluded(i..self.bytes.len());
                                        i = self.bytes.len();
                                    }
                                }
                            }
                        }
                        None => i += 1,
                    }
                }
                b'!' if self.bytes.get(i + 1) == Some(&b'[') => {
                    if let Some(next) = self.try_wiki(i + 1, true) {
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                b'[' if self.bytes.get(i + 1) == Some(&b'[') => {
                    if let Some(next) = self.try_wiki(i, false) {
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                b'[' => {
                    // Fail fast on a construct the budget no longer covers
                    // (R13-1: repeated failed attempts are the hostile
                    // shape; see `search_debt_cap`).
                    if !self.charge_debt(0) {
                        i += 1;
                        continue;
                    }
                    if let Some(next) = self.try_markdown_link(i) {
                        i = next;
                    } else {
                        i += 1;
                    }
                }
                b'\\' => i += 2,
                _ => i += 1,
            }
        }
    }

    /// Charge `searched` bytes of construct-search debt. Returns false
    /// when the budget is exhausted (the caller stops attempting
    /// constructs but keeps walking monotonically). `searched == 0` is a
    /// pure budget probe used before attempting a construct.
    fn charge_debt(&mut self, searched: usize) -> bool {
        self.search_debt = self.search_debt.saturating_add(searched);
        if self.search_debt > search_debt_cap(self.source.len()) {
            self.degraded = true;
            return false;
        }
        true
    }

    /// Record an exclusion range, keeping `excluded` sorted by start and
    /// coalescing touching/overlapping ranges. With coalescing, the
    /// `in_excluded` binary search stays O(log n) and the vector stays
    /// proportional to DISJOINT spans, not to the number of constructs.
    fn push_excluded(&mut self, range: std::ops::Range<usize>) {
        if range.start >= range.end {
            return;
        }
        if let Some(last) = self.excluded.last_mut() {
            if range.start <= last.end {
                if range.end > last.end {
                    last.end = range.end;
                }
                return;
            }
        }
        self.excluded.push(range);
    }

    /// Inclusion probe for [start, end) — O(log n) over coalesced ranges.
    fn in_excluded(&self, start: usize, end: usize) -> bool {
        let idx = self.excluded.partition_point(|r| r.end <= start);
        self.excluded
            .get(idx)
            .is_some_and(|r| start < r.end && end > r.start)
    }

    fn push_token(&mut self, token: LinkToken) {
        if self.tokens.len() >= self.max_links {
            self.link_capped = true;
            return;
        }
        self.tokens.push(token);
    }

    /// `[[path#sub|display]]`; `open` points at the first `[`.
    fn try_wiki(&mut self, open: usize, embed: bool) -> Option<usize> {
        let inner_start = open + 2;
        // Failed searches are the hostile-input hot path (`[[` flood):
        // bound one attempt's scan distance and charge it to the debt
        // budget so the walk stays linear overall.
        let window_end = (inner_start + MAX_CONSTRUCT_RESCAN).min(self.source.len());
        let window = self.source.get(inner_start..window_end)?;
        let close_rel = window.find("]]")?;
        if !self.charge_debt(close_rel.saturating_sub(2).max(4)) {
            return None;
        }
        let close = inner_start + close_rel;
        // A wiki link never spans lines and its inner text never contains
        // another `[[` (Obsidian emits single-line links; anything else is
        // plain text, e.g. the unclosed-construct case).
        if self.source[inner_start..close].contains('\n')
            || self.source[inner_start..close].contains("[[")
        {
            return None;
        }
        let inner = &self.source[inner_start..close];
        if inner.is_empty() {
            return None;
        }
        let (path, rest) = match inner.find('|') {
            Some(pipe) => (&inner[..pipe], &inner[pipe..]),
            None => (inner, ""),
        };
        let (path, subpath) = match path.find('#') {
            Some(hash) => (&path[..hash], &path[hash..]),
            None => (path, ""),
        };
        let rel = |slice: &str| inner_start + (slice.as_ptr() as usize - inner.as_ptr() as usize);
        if path.trim().is_empty() && subpath.is_empty() {
            return None;
        }
        let token = LinkToken {
            whole: if embed { open - 1 } else { open }..close + 2,
            path: rel(path)..rel(path) + path.len(),
            subpath: (!subpath.is_empty()).then(|| {
                let s = rel(subpath);
                s..s + subpath.len()
            }),
            display: (!rest.is_empty()).then(|| {
                let s = rel(rest);
                s..s + rest.len()
            }),
            syntax: Syntax::Wiki,
            embed,
            host: Host::Body,
        };
        self.push_token(token);
        Some(close + 2)
    }

    /// `[label](dest "title")` with escapes and nested brackets/parens.
    fn try_markdown_link(&mut self, open: usize) -> Option<usize> {
        let label_end = match self.match_brackets(open) {
            Some(end) => end,
            None => {
                // Unclosed brackets scanned to EOF — that walk is the
                // hostile-input cost center; charge it even on failure
                // (R13-1) so a `[` flood cannot go quadratic.
                let scanned = self.bytes.len().saturating_sub(open);
                if !self.charge_debt(scanned) {
                    return None;
                }
                return None;
            }
        };
        if !self.charge_debt(label_end.saturating_sub(open).max(4)) {
            return None;
        }
        if self.bytes.get(label_end) != Some(&b'(') {
            return None;
        }
        let (close, dest_range) = self.match_destination(label_end + 1)?;
        if !self.charge_debt(close.saturating_sub(label_end).max(4)) {
            return None;
        }
        let dest = &self.source[dest_range.clone()];
        if dest.is_empty() {
            return None;
        }
        if !markdown_dest_is_relative(dest) {
            return Some(close + 1);
        }
        let token = LinkToken {
            whole: open..close + 1,
            path: dest_range,
            subpath: None,
            display: None,
            syntax: Syntax::Markdown,
            embed: false,
            host: Host::Body,
        };
        self.push_token(token);
        Some(close + 1)
    }

    fn match_brackets(&self, open: usize) -> Option<usize> {
        let mut depth = 0usize;
        let mut i = open;
        while i < self.bytes.len() {
            match self.bytes[i] {
                b'\\' => i += 2,
                b'[' => {
                    depth += 1;
                    i += 1;
                }
                b']' => {
                    depth -= 1;
                    i += 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => i += 1,
            }
        }
        None
    }

    fn match_destination(&self, open: usize) -> Option<(usize, std::ops::Range<usize>)> {
        if self.bytes.get(open) == Some(&b'<') {
            let mut j = open + 1;
            while j < self.bytes.len() {
                match self.bytes[j] {
                    b'\\' => j += 2,
                    b'>' => {
                        let close = self.find_closing_paren(j + 1)?;
                        return Some((close, open + 1..j));
                    }
                    b'\n' => break,
                    _ => j += 1,
                }
            }
        }
        let mut depth = 1usize;
        let mut end = open;
        let mut j = open;
        while j < self.bytes.len() {
            match self.bytes[j] {
                b'\\' => j += 2,
                b'(' => {
                    depth += 1;
                    j += 1;
                }
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some((j, open..end));
                    }
                    j += 1;
                }
                b' ' | b'\t' | b'\n' => {
                    let close = self.find_closing_paren(j)?;
                    return Some((close, open..end));
                }
                _ => {
                    j += 1;
                    end = j;
                }
            }
        }
        None
    }

    fn find_closing_paren(&self, from: usize) -> Option<usize> {
        let mut depth = 1usize;
        let mut j = from;
        while j < self.bytes.len() {
            match self.bytes[j] {
                b'\\' => j += 2,
                b'(' => {
                    depth += 1;
                    j += 1;
                }
                b')' => {
                    depth -= 1;
                    j += 1;
                    if depth == 0 {
                        return Some(j - 1);
                    }
                }
                _ => j += 1,
            }
        }
        None
    }
}

fn fence_marker(line: &str) -> Option<(u8, usize)> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let bytes = trimmed.as_bytes();
    let backticks = bytes.iter().take_while(|&&b| b == b'`').count();
    if backticks >= 3 {
        return Some((b'`', backticks));
    }
    let tildes = bytes.iter().take_while(|&&b| b == b'~').count();
    if tildes >= 3 {
        return Some((b'~', tildes));
    }
    None
}

fn backtick_run(bytes: &[u8], at: usize) -> usize {
    bytes[at..].iter().take_while(|&&b| b == b'`').count()
}

/// True when the byte at `at` is preceded by an odd number of backslashes
/// (i.e. it is itself escaped).
fn escaped_at(source: &[u8], at: usize) -> bool {
    source[..at]
        .iter()
        .rev()
        .take_while(|&&b| b == b'\\')
        .count()
        % 2
        == 1
}

fn find_backtick_close(bytes: &[u8], from: usize, len: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if run == len {
                return Some(i + run);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

fn markdown_dest_is_relative(dest: &str) -> bool {
    if dest.is_empty() || dest.starts_with('/') || dest.starts_with('\\') {
        return false;
    }
    if let Some(colon) = dest.find(':') {
        let before = &dest[..colon];
        if !before.is_empty() && before.bytes().all(|b| b.is_ascii_alphabetic()) {
            return false;
        }
    }
    true
}
