//! Static analysis of a jq program: does it read its input, and which
//! members of which variables does it reach?
//!
//! Everything here works on jaq's parse tree, before compilation, so a
//! program that uses a filter jaq does not define can still be analyzed.
//! The analysis is conservative: whenever it cannot prove that the input is
//! left alone, or that a variable is only accessed by literal keys, it says
//! so, and the caller has to assume the worst.

use std::collections::BTreeSet;

use jaq_all::jaq_core::load::lex::StrPart;
use jaq_all::jaq_core::load::parse::{BinaryOp, Def, Pattern, Term};
use jaq_all::jaq_core::load::{self, lex, parse, File};
use jaq_all::jaq_core::path::{Part, Path};

use crate::errors::render;

/// How a program uses one variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Members {
    /// Only ever accessed as `$x.key` / `$x["key"]` with literal keys.
    Keys(BTreeSet<String>),
    /// Used in some other way (`$x`, `$x | f`, `$x[$k]`, ...).
    Whole,
}

#[derive(Debug)]
pub(crate) struct Analysis {
    pub(crate) reads_input: bool,
    /// Every free variable (used but not bound by the program itself),
    /// without the leading `$`, in order of first appearance.
    pub(crate) vars: Vec<(String, Members)>,
}

/// Zero-argument filters that never look at their input.
///
/// `true` and `false` are `def`s over `0 == 0`; the rest are natives.
/// Anything not listed (including `not`, `length`, `error`, formats such as
/// `@base64`) is assumed to read its input.
const INPUT_FREE_CALLS: &[&str] = &[
    "empty",
    "env",
    "false",
    "input_filename",
    "now",
    "null",
    "true",
];

pub(crate) fn analyze(code: &str) -> Result<Analysis, String> {
    let file = || File { path: (), code };
    let tokens = lex::Lexer::new(code).lex().map_err(|e| {
        render(jaq_all::load::load_errors(vec![(
            file(),
            load::Error::Lex(e),
        )]))
    })?;
    let term = parse::Parser::new(skip_module_header(&tokens))
        .parse(|p| p.term())
        .map_err(|e| {
            let errs = e
                .into_iter()
                .map(|(expected, found)| (expected, lex::Token::opt_as_str(found, code)))
                .collect();
            render(jaq_all::load::load_errors(vec![(
                file(),
                load::Error::Parse(errs),
            )]))
        })?;

    let mut collector = Collector {
        bound: Vec::new(),
        vars: Vec::new(),
    };
    collector.term(&term);
    Ok(Analysis {
        reads_input: reads_input(&term, &[]),
        vars: collector.vars,
    })
}

/// Drop a leading `module <meta>;`, which `compile()` accepts and ignores.
///
/// jaq's module parser is private, so the header is skipped on the token
/// level: it ends at the first top-level `;` that does not close a `def`
/// inside the metadata. `include` / `import` are left in place; they fail
/// to parse here just as they fail to load in `compile()`.
fn skip_module_header<'t, 's>(tokens: &'t [lex::Token<&'s str>]) -> &'t [lex::Token<&'s str>] {
    let [lex::Token("module", lex::Tok::Word), rest @ ..] = tokens else {
        return tokens;
    };
    let mut open = 1;
    for (i, lex::Token(s, _)) in rest.iter().enumerate() {
        match *s {
            "def" => open += 1,
            ";" => {
                open -= 1;
                if open == 0 {
                    return &rest[i + 1..];
                }
            }
            _ => {}
        }
    }
    // Unterminated: let the parser report the error on the whole input.
    tokens
}

/// Whether evaluating `t` can look at the value it is applied to.
///
/// `local` holds the zero-argument `def`s in scope, which shadow the
/// input-free builtins of the same name.
fn reads_input(t: &Term<&str>, local: &[&str]) -> bool {
    match t {
        Term::Id | Term::Recurse => true,
        Term::Var(_) | Term::Num(_) | Term::Break(_) => false,
        // A format with string parts (`@base64 "\(x)"`) applies only to the
        // interpolations; a bare format is a `Call` and handled below.
        Term::Str(_, parts) => parts.iter().any(|part| match part {
            StrPart::Term(t) => reads_input(t, local),
            StrPart::Str(_) | StrPart::Char(_) => false,
        }),
        Term::Arr(t) => t.as_deref().is_some_and(|t| reads_input(t, local)),
        // `{a}` and `{"a"}` are `{a: .a}`; `{$x}` is `{x: $x}`.
        Term::Obj(entries) => entries.iter().any(|(k, v)| match v {
            Some(v) => reads_input(k, local) || reads_input(v, local),
            None => !matches!(k, Term::Var(_)),
        }),
        Term::Neg(t) | Term::Label(_, t) => reads_input(t, local),
        // The right-hand side of a plain pipe sees the left's output, not
        // the input; `l as $x | r` runs `r` on the original input.
        Term::BinOp(l, BinaryOp::Pipe(None), _) => reads_input(l, local),
        Term::BinOp(l, BinaryOp::Pipe(Some(pat)), r) => {
            reads_input(l, local) || reads_input(r, local) || pattern_reads_input(pat, local)
        }
        Term::BinOp(l, _, r) => reads_input(l, local) || reads_input(r, local),
        // `init` runs on the input; `update` / `extract` on the accumulator.
        Term::Fold(_, xs, pat, args) => {
            reads_input(xs, local)
                || pattern_reads_input(pat, local)
                || args.first().is_some_and(|t| reads_input(t, local))
        }
        // `catch` runs on the error, not on the input.
        Term::TryCatch(t, _) => reads_input(t, local),
        // Without `else`, a false condition yields `.`.
        Term::IfThenElse(branches, else_) => match else_ {
            None => true,
            Some(else_) => {
                reads_input(else_, local)
                    || branches
                        .iter()
                        .any(|(c, t)| reads_input(c, local) || reads_input(t, local))
            }
        },
        // Calls to a local `def` count as reads, so only the body itself
        // needs looking at, once the new names shadow any builtins.
        Term::Def(defs, body) => {
            let mut local = local.to_vec();
            local.extend(defs.iter().filter(|d| d.args.is_empty()).map(|d| d.name));
            reads_input(body, &local)
        }
        Term::Call(name, args) => {
            !(args.is_empty() && INPUT_FREE_CALLS.contains(name) && !local.contains(name))
        }
        // Index and range terms (`$x[.k]`) run on the input too.
        Term::Path(base, Path(parts)) => {
            reads_input(base, local) || parts.iter().any(|(p, _)| part_reads_input(p, local))
        }
    }
}

fn part_reads_input(part: &Part<Term<&str>>, local: &[&str]) -> bool {
    match part {
        Part::Index(t) => reads_input(t, local),
        Part::Range(from, to) => {
            from.as_ref().is_some_and(|t| reads_input(t, local))
                || to.as_ref().is_some_and(|t| reads_input(t, local))
        }
    }
}

/// Computed keys in a destructuring pattern (`{(f): $x}`) — conservative.
fn pattern_reads_input(pat: &Pattern<&str>, local: &[&str]) -> bool {
    match pat {
        Pattern::Var(_) => false,
        Pattern::Arr(pats) => pats.iter().any(|p| pattern_reads_input(p, local)),
        Pattern::Obj(entries) => entries
            .iter()
            .any(|(k, p)| reads_input(k, local) || pattern_reads_input(p, local)),
    }
}

/// The literal text of a string term without interpolation, if it is one.
fn literal_key(t: &Term<&str>) -> Option<String> {
    let Term::Str(None, parts) = t else {
        return None;
    };
    let mut key = String::new();
    for part in parts {
        match part {
            StrPart::Str(s) => key.push_str(s),
            StrPart::Char(c) => key.push(*c),
            StrPart::Term(_) => return None,
        }
    }
    Some(key)
}

fn record(vars: &mut Vec<(String, Members)>, name: &str, member: Option<String>) {
    let name = name.trim_start_matches('$');
    let slot = match vars.iter_mut().find(|(n, _)| n == name) {
        Some((_, slot)) => slot,
        None => {
            vars.push((name.to_owned(), Members::Keys(BTreeSet::new())));
            &mut vars.last_mut().unwrap().1
        }
    };
    match (slot, member) {
        (Members::Keys(keys), Some(member)) => {
            keys.insert(member);
        }
        (slot, _) => *slot = Members::Whole,
    }
}

/// Collects the variables a term uses without binding them itself.
///
/// `bound` is the stack of names bound by enclosing `as`, `reduce`,
/// `foreach` and `def f($x):`, each with its leading `$`.
struct Collector<'s> {
    bound: Vec<&'s str>,
    vars: Vec<(String, Members)>,
}

impl<'s> Collector<'s> {
    fn var(&mut self, x: &'s str, member: Option<String>) {
        if !self.bound.contains(&x) {
            record(&mut self.vars, x, member);
        }
    }

    /// Run `f` with the variables bound by `pat` in scope.
    fn with_pattern(&mut self, pat: &Pattern<&'s str>, f: impl FnOnce(&mut Self)) {
        let depth = self.bound.len();
        self.bind(pat);
        f(self);
        self.bound.truncate(depth);
    }

    fn bind(&mut self, pat: &Pattern<&'s str>) {
        match pat {
            Pattern::Var(x) => self.bound.push(x),
            Pattern::Arr(pats) => pats.iter().for_each(|p| self.bind(p)),
            Pattern::Obj(entries) => entries.iter().for_each(|(_, p)| self.bind(p)),
        }
    }

    /// Computed keys of a pattern (`{(f): $x}`) are evaluated in the
    /// enclosing scope; the pattern's own `$x` are bindings, not uses.
    fn pattern_keys(&mut self, pat: &Pattern<&'s str>) {
        match pat {
            Pattern::Var(_) => {}
            Pattern::Arr(pats) => pats.iter().for_each(|p| self.pattern_keys(p)),
            Pattern::Obj(entries) => entries.iter().for_each(|(k, p)| {
                self.term(k);
                self.pattern_keys(p);
            }),
        }
    }

    fn def(&mut self, def: &Def<&'s str>) {
        let depth = self.bound.len();
        self.bound
            .extend(def.args.iter().copied().filter(|a| a.starts_with('$')));
        self.term(&def.body);
        self.bound.truncate(depth);
    }

    fn term(&mut self, t: &Term<&'s str>) {
        match t {
            Term::Var(x) => self.var(x, None),
            Term::Path(base, Path(parts)) => {
                let mut rest = parts.iter();
                if let Term::Var(x) = &**base {
                    match parts.first() {
                        Some((Part::Index(k), _)) if literal_key(k).is_some() => {
                            self.var(x, literal_key(k));
                            rest.next();
                        }
                        _ => self.var(x, None),
                    }
                } else {
                    self.term(base);
                }
                for (part, _) in rest {
                    match part {
                        Part::Index(t) => self.term(t),
                        Part::Range(from, to) => from.iter().chain(to).for_each(|t| self.term(t)),
                    }
                }
            }
            Term::Id | Term::Recurse | Term::Num(_) | Term::Break(_) => {}
            Term::Str(_, parts) => parts.iter().for_each(|part| {
                if let StrPart::Term(t) = part {
                    self.term(t);
                }
            }),
            Term::Arr(t) => t.iter().for_each(|t| self.term(t)),
            Term::Obj(entries) => entries.iter().for_each(|(k, v)| {
                self.term(k);
                v.iter().for_each(|v| self.term(v));
            }),
            Term::Neg(t) | Term::Label(_, t) => self.term(t),
            Term::BinOp(l, BinaryOp::Pipe(Some(pat)), r) => {
                self.term(l);
                self.pattern_keys(pat);
                self.with_pattern(pat, |c| c.term(r));
            }
            Term::BinOp(l, _, r) => {
                self.term(l);
                self.term(r);
            }
            // `init` is outside the binding; `update` and `extract` inside.
            Term::Fold(_, xs, pat, args) => {
                self.term(xs);
                self.pattern_keys(pat);
                let mut args = args.iter();
                if let Some(init) = args.next() {
                    self.term(init);
                }
                self.with_pattern(pat, |c| args.for_each(|t| c.term(t)));
            }
            Term::TryCatch(t, catch) => {
                self.term(t);
                catch.iter().for_each(|c| self.term(c));
            }
            Term::IfThenElse(branches, else_) => {
                branches.iter().for_each(|(c, t)| {
                    self.term(c);
                    self.term(t);
                });
                else_.iter().for_each(|t| self.term(t));
            }
            Term::Def(defs, body) => {
                defs.iter().for_each(|d| self.def(d));
                self.term(body);
            }
            Term::Call(_, args) => args.iter().for_each(|t| self.term(t)),
        }
    }
}
