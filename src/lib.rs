extern crate serde;
#[macro_use]
extern crate serde_derive;

// TODO(high): insert a single blank entry/line, treat it specially, such that enter on it just exits is ignored but enter on other chars actually enters them
// TODO(high): fix enter giving \n bug
// TODO(high): fix delimiter to not act up on very long strings
// TODO(high): use library instead of hardcoded ANSI codes
// TODO(mid): de-dup parentheticals in POJ

const WORD_WIDTH: usize = 15;
const DELIM: &str = "  ";
// TODO: Determine dynamically, and/or fetch from https://github.com/ChhoeTaigi/ChhoeTaigiDatabase/blob/master/ChhoeTaigiDatabase/ChhoeTaigi_MaryknollTaiengSutian.csv
const FILENAME: &str = "/home/glenn/ah_taigi/ChhoeTaigiDatabase/ChhoeTaigiDatabase/ChhoeTaigi_MaryknollTaiengSutian.csv";

use std::io;
use std::io::prelude::*;

use std::process::{Command, Stdio};

use std::fmt;
use std::fmt::Display;

use std::error::Error;
use std::fs::File;

#[derive(Clone, Debug, Deserialize)]
struct TaigiEntry {
    poj_unicode: String,
    english: String,
}

#[derive(Clone, Debug)]
struct TaigiEntryBag {
    entries: Vec<TaigiEntry>,
    cached_display_string: String,
}

impl TaigiEntryBag {
    fn new(entries: Vec<TaigiEntry>) -> Self {
        let cached_display_string = Self::format_for_selection(&entries);

        Self {
            entries,
            cached_display_string,
        }
    }

    fn format_for_selection(entries: &[TaigiEntry]) -> String {
        entries
            .iter()
            .map(Self::format_str)
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn format_str(entry: &TaigiEntry) -> String {
        format!(
            "\x1B[1m{poj}\x1B[0m{delim}\x1B[3m{blank:width$}{eng}\x1B[0m",
            poj=entry.poj_unicode, 
            delim=DELIM, 
            blank="",
            eng=entry.english,
            width=WORD_WIDTH.saturating_sub(entry.poj_unicode.chars().count()),
        )
    }

    fn get_selector_input(&self) -> &str {
        self.cached_display_string.as_ref()
    }

    fn get_entries_from_selection_output(&self, output: &str) -> io::Result<Vec<TaigiEntry>> {
        let mut entries = vec![];
        let str_indices = output.split(" ");
        for s in str_indices {
            let index = s.parse::<usize>().or_else(|_| Err(get_parse_err(output)))?;
            let entry = self.entries.get(index).map(Clone::clone).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::Other,
                    format!("No entry matching {} found.", index),
                )
            })?;
            entries.push(entry);
        }
        Ok(entries)
    }
}

fn get_parse_err(output: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::Other,
        format!("Could not parse output from selection program: {}", output),
    )
}

impl Display for TaigiEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.poj_unicode)
    }
}

pub fn run() -> io::Result<()> {
    let entries = read_entries().unwrap();
    let entries = TaigiEntryBag::new(entries);
    let selector_stdin = entries.get_selector_input();
    let raw_text_output = run_fzf(selector_stdin)?;
    let selected_entries = entries.get_entries_from_selection_output(&raw_text_output)?;

    let to_print = selected_entries
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");

    println!("{}", to_print);

    Ok(())
}

fn read_entries() -> Result<Vec<TaigiEntry>, Box<dyn Error>> {
    let f = File::open(FILENAME).unwrap();
    let mut reader = csv::Reader::from_reader(f);
    let mut results = vec![];
    for result in reader.deserialize() {
        let res: TaigiEntry = result?;

        if !res.poj_unicode.contains(" ") {
            results.push(res);
        }
    }
    Ok(results)
}

fn run_fzf(selector_stdin: &str) -> io::Result<String> {
    let mut cmd = get_fzf_command();
    let mut child = cmd.spawn()?;

    let stdin = child.stdin.as_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Other,
            "Could not acquire write access to stdin.",
        )
    })?;

    stdin.write_all(selector_stdin.as_bytes())?;

    let res = child.wait_with_output();

    match res {
        Ok(output) => {
            // TODO: check if number output came out, ignore success/failure code
            Ok(String::from_utf8_lossy(&output.stdout)
                .trim_end_matches(|x| x == '\n')
                .to_string())
        }
        Err(err) => Err(err),
    }
}

fn get_fzf_command() -> Command {
    let mut cmd = Command::new("fzf");

    let preview_command = &construct_preview_command();

    let args = vec![
        "--ansi",
        "--reverse",
        "--multi",
        "--bind",
        "tab:toggle+down+clear-query+first",
        "--bind",
        "enter:execute(echo {+n})+cancel",
        "--preview",
        &preview_command,
        "--preview-window",
        "up:1",
    ];

    cmd.args(args)
        .stdin(Stdio::piped())
        // Taking stderr breaks fzf.
        //.stderr(Stdio::piped())
        .stdout(Stdio::piped());
    cmd
}

fn construct_preview_command() -> String {
    let awk_cmd = format!(
        r#"awk -F "{delim}" "{{printf \"%s \",\$1}}""#,
        delim = DELIM
    );

    let current_word_cmd = format!(
        r#"
            echo -en "\e[1m";
            echo -n {{}} | {awk_cmd}; 
            echo -en "\e[0m";
            "#,
        awk_cmd = awk_cmd
    );

    // NOTE: This if is neccessary because {+} is equal to the query if nothing is selected
    format!(
        r#"
        if [[ "{{+}}" == "{{}}" ]]; then
            {current_word_cmd}
        else
            for selection in {{+}}; 
                do echo -n "$selection" | {awk_cmd}; 
            done
            {current_word_cmd}
        fi"#,
        awk_cmd = awk_cmd,
        current_word_cmd = current_word_cmd
    )
}
