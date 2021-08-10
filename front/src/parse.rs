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
    /// Constructor.
    pub fn new(reader: R) -> Res<Self> {
        let mut reader = BufReader::new(reader);
        let mut line_buf = String::with_capacity(17);
        reader
            .read_line(&mut line_buf)
            .chain_err(|| "while reading first line")?;

        const PREF: &str = "p cnf ";

        macro_rules! bail {
            {} => {
                return Err(format!(
                    "[{}:{}] error on first line, expected `{}<int> <int>` format",
                    file!(),
                    line!(),
                    PREF,
                ).into());
            };
        }

        log::trace!("parsing first CNF line");

        if line_buf.len() < PREF.len() {
            bail!()
        } else if &line_buf[0..PREF.len()] != PREF {
            bail!()
        }

        log::trace!("prefix okay");

        let mut start = PREF.len();
        let mut cnt = start;
        let mut chars = line_buf[cnt..].chars();
        let lit_count = {
            while let Some(c) = chars.next() {
                cnt += 1;
                if c.is_ascii_digit() {
                    continue;
                } else if c == ' ' {
                    break;
                } else {
                    bail!()
                }
            }
            if cnt == start {
                bail!()
            }
            let lit_count = &line_buf[start..cnt - 1];
            log::trace!("lit_count substring: {:?}", lit_count);
            if let Ok(res) = usize::from_str_radix(lit_count, 10) {
                res
            } else {
                bail!()
            }
        };
        log::trace!("lit_count is {}", lit_count);
        let disj_count = {
            start = cnt;
            if start > line_buf.len() {
                bail!()
            }
            cnt = start;
            while let Some(c) = chars.next() {
                cnt += 1;
                if c.is_ascii_digit() {
                    continue;
                } else if c == '\n' {
                    // This should end the line, meaning we automatically get `None` next. Just
                    // continueing here to let the normal workflow handle this.
                    continue;
                } else {
                    log::error!("unexpected character `{}`", c);
                    bail!()
                }
            }
            if cnt == start {
                bail!()
            }
            let disj_count = &line_buf[start..cnt - 1];
            log::trace!("disj_count substring: {:?}", disj_count);
            if let Ok(res) = usize::from_str_radix(disj_count, 10) {
                res
            } else {
                bail!()
            }
        };
        log::trace!("disj_count is {}", disj_count);

        Ok(Self {
            reader,
            line_buf,
            line: 0,
            lit_count,
            cnf: Cnf::with_capacity(disj_count),
        })
    }

    pub fn fail(&self, msg: impl Display) -> err::Error {
        format!("error line {}: {}", self.line + 1, msg).into()
    }

    fn parse_clause(&mut self) -> Res<()> {
        let mut mini_parser = DisjParser::new(&self.line_buf);
        let mut clause = Clause::with_capacity(7);
        // Line loaded.
        'read_lit: loop {
            match mini_parser.lit()? {
                Some(lit) => {
                    log::trace!("parsed a lit: {}", lit);
                    clause.push(lit);
                    mini_parser.space()?;
                }
                None => break 'read_lit,
            }
        }
        self.cnf.push(clause);
        Ok(())
    }

    pub fn parse(mut self) -> Res<Cnf<Lit>> {
        loop {
            self.line += 1;
            log::trace!("parsing line {}", self.line);
            self.line_buf.clear();
            let read = self.reader.read_line(&mut self.line_buf)?;
            if read == 0 {
                // EOF reached.
                break;
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

    fn space(&mut self) -> Res<()> {
        const ERR: &str = "expected space character ` `, got end of line";
        let char = self.txt[self.cursor..].chars().next().ok_or(ERR)?;
        if char != ' ' {
            bail!(ERR)
        }
        self.cursor += 1;
        Ok(())
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
