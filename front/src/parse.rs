//! SAT-comp format parser.

use std::{
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Read},
    path::Path,
};

use xz2::bufread::XzDecoder;

prelude!();

/// SAT-comp CNF parser.
pub struct Parser<R: Read> {
    reader: BufReader<R>,
    line_buf: String,
    line: usize,
    #[allow(dead_code)]
    lit_count: usize,
    cnf: Cnf<Lit>,
}

impl Parser<File> {
    pub fn open_file(path: impl AsRef<Path>) -> Res<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .chain_err(|| format!("while opening file `{}`", path.display()))?;
        Self::new(file)
    }
}
impl Parser<XzDecoder<BufReader<File>>> {
    pub fn open_xz_file(path: impl AsRef<Path>) -> Res<Self> {
        let path = path.as_ref();
        let file = OpenOptions::new()
            .read(true)
            .open(path)
            .chain_err(|| format!("while opening file `{}`", path.display()))?;
        Self::new(XzDecoder::new(BufReader::new(file)))
    }
}

impl<R: Read> Parser<R> {
    /// Puts the first line from `reader` that's not a comment in `line_buf`.
    ///
    /// Clears `line_buf`.
    ///
    /// Return the number of comment lines read, or `0` if EOI was reached, potentially after
    /// parsing some comment lines.
    fn read_line(reader: &mut BufReader<R>, line_buf: &mut String) -> Res<usize> {
        let mut cnt = 0;
        loop {
            line_buf.clear();
            let bytes_read = reader
                .read_line(line_buf)
                .chain_err(|| "while reading first line")?;
            if bytes_read == 0 || line_buf.trim() == "0" {
                break Ok(0);
            } else {
                cnt += 1;
                if !line_buf.is_empty() && (&line_buf[0..1] == "c" || &line_buf[0..1] == "%") {
                    // Comment line, move on.
                    continue;
                } else {
                    break Ok(cnt);
                }
            }
        }
    }
    /// Constructor.
    pub fn new(reader: R) -> Res<Self> {
        let mut reader = BufReader::new(reader);
        let mut line_buf = String::with_capacity(17);

        const PREF: &str = "p cnf";

        macro_rules! err {
            {} => {
                format!(
                    "error on first non-comment line, expected `{}<int> <int>` format", PREF,
                )
            };
        }
        macro_rules! bail {
            {} => {
                return Err(err!().into())
            };
        }

        let lines_read = Self::read_line(&mut reader, &mut line_buf)?;
        if lines_read == 0 {
            bail!()
        }

        log::trace!("parsing first CNF line");

        if line_buf.len() < PREF.len() {
            bail!()
        } else if &line_buf[0..PREF.len()] != PREF {
            bail!()
        }

        log::trace!("prefix okay");

        let start = PREF.len();

        let txt = &line_buf[start..];
        log::trace!("parsing tail `{}`", txt.trim());
        let mut parser = DisjParser::new(txt);
        parser.space(1).chain_err(|| err!())?;
        let lit_count = parser.usize().chain_err(|| err!())?;
        log::trace!("lit_count is {}", lit_count);
        log::trace!("parsing tail `{}`", parser.txt.trim());
        parser.space(1).chain_err(|| err!())?;
        let disj_count = parser.usize().chain_err(|| err!())?;
        log::trace!("disj_count is {}", disj_count);

        Ok(Self {
            reader,
            line_buf,
            line: lines_read,
            lit_count,
            cnf: Cnf::with_capacity(disj_count),
        })
    }

    pub fn fail(&self, msg: impl Display) -> err::Error {
        format!(
            "error line {}: {} `{}`",
            self.line,
            msg,
            self.line_buf.trim()
        )
        .into()
    }

    fn parse_clause(&mut self) -> Res<()> {
        let mut mini_parser = DisjParser::new(&self.line_buf);
        let mut clause = Clause::with_capacity(7);
        mini_parser.space(0)?;
        // Line loaded.
        'read_lit: loop {
            match mini_parser.lit()? {
                Some(lit) => {
                    log::trace!("parsed a lit: {}", lit);
                    clause.push(lit);
                    mini_parser.space(1)?;
                }
                None => break 'read_lit,
            }
        }
        self.cnf.push(clause);
        Ok(())
    }

    pub fn parse(mut self) -> Res<Cnf<Lit>> {
        loop {
            log::trace!("parsing line {}", self.line);
            self.line_buf.clear();
            let lines_read = Self::read_line(&mut self.reader, &mut self.line_buf)?;
            if lines_read == 0 {
                // EOF reached.
                break;
            } else {
                self.line += lines_read;
            }

            self.parse_clause()
                .chain_err(|| self.fail("while parsing this line"))?;
        }
        Ok(self.cnf)
    }
}

struct DisjParser<'txt> {
    txt: &'txt str,
    cursor: usize,
}
impl<'txt> DisjParser<'txt> {
    fn new(txt: &'txt str) -> Self {
        Self { txt, cursor: 0 }
    }

    fn space(&mut self, min: usize) -> Res<()> {
        for (idx, c) in self.txt[self.cursor..].chars().enumerate() {
            if c.is_whitespace() {
                self.cursor += c.len_utf8()
            } else if idx < min {
                bail!("expected space character ` `, got end of line")
            } else {
                break;
            }
        }
        Ok(())
    }
    fn usize(&mut self) -> Res<usize> {
        let end = self.txt[self.cursor..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .fold(self.cursor, |acc, c| acc + c.len_utf8());
        let n = usize::from_str_radix(&self.txt[self.cursor..end], 10)
            .chain_err(|| "illegal usize value")?;
        self.cursor = end;
        Ok(n)
    }
    fn lit(&mut self) -> Res<Option<Lit>> {
        let (start, negated) = match self.txt[self.cursor..].chars().next() {
            Some('-') => (self.cursor + 1, true),
            Some(c) if c.is_ascii_digit() => (self.cursor, false),
            Some(c) => bail!("expected literal (`-` or digit), got `{}`", c),
            None => bail!("expected literal, got end of line"),
        };
        self.cursor = start;
        let mut chars = self.txt[start..].chars();
        loop {
            match chars.next() {
                Some(c) if c.is_ascii_digit() => self.cursor += 1,
                None | Some(_) => break,
            }
        }
        let idx_str = &self.txt[start..self.cursor];
        let idx = match usize::from_str_radix(idx_str, 10) {
            Ok(idx) => idx,
            Err(_) => bail!("expected literal UID, found `{}`", idx_str),
        };
        if idx == 0 {
            if negated {
                bail!("unexpected negated `0`, illegal end of line marker")
            }
            Ok(None)
        } else {
            Ok(Some(Lit::new(idx, negated)))
        }
    }
}
