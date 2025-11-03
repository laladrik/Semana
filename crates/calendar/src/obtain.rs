use crate::DateStream;

use super::{Date, Item, Time};
use std::{ffi::OsStr, str::FromStr};
pub trait AgendaSource {
    type Data;
    type Error;
    fn obtain<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Self::Data, Self::Error>;
}

pub struct AgendaSourceStd;

impl AgendaSource for AgendaSourceStd {
    type Data = Vec<u8>;
    type Error = std::io::Error;

    fn obtain<S: AsRef<OsStr>>(&self, args: &[S]) -> Result<Self::Data, Self::Error> {
        use std::process;
        let mut cmd = process::Command::new(&args[0]);
        cmd.args(args[1..].iter());
        cmd.stdout(process::Stdio::piped());
        let child: process::Child = cmd.spawn()?;
        let output: process::Output = child.wait_with_output()?;
        if !output.status.success() {
            panic!("the command failed");
        }
        Ok(output.stdout)
    }
}

pub trait JsonParser {
    type Error;

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<Agenda, Self::Error>;
}

pub struct NanoSerde;
impl JsonParser for NanoSerde {
    type Error = nanoserde::DeJsonErr;

    fn parse<'data, 'me: 'data>(&'me self, bytes: &'data str) -> Result<Agenda, Self::Error> {
        nanoserde::DeJson::deserialize_json(bytes)
    }
}

pub type Agenda = Vec<Item>;

#[derive(Debug)]
pub enum Error<PE> {
    Io(std::io::Error),
    InvalidUnicode(core::str::Utf8Error),
    Parse(PE),
    DurationIsTooBig,
}

const MAX_DURATION_DAYS: u8 = 35;
pub mod khal {
    use super::ObtainArguments;
    pub fn week_arguments(from: &str) -> ObtainArguments<'_> {
        ObtainArguments {
            from,
            duration_days: 7,
            backend_bin_path: "khal",
        }
    }
}

pub struct ObtainArguments<'s> {
    // date in the format YYYY-MM-DD
    pub from: &'s str,
    // date in the format YYYY-MM-DD
    pub duration_days: u8,
    // path to khal
    pub backend_bin_path: &'s str,
}

pub fn obtain<AS, JP, O>(
    agenda_source: &AS,
    json_parser: &JP,
    arguments: &ObtainArguments,
) -> Result<Agenda, Error<JP::Error>>
where
    AS: AgendaSource<Data = O, Error = std::io::Error>,
    JP: JsonParser,
    O: AsRef<[u8]>,
{
    if arguments.duration_days > MAX_DURATION_DAYS {
        return Err(Error::DurationIsTooBig);
    }

    let args = [
        arguments.backend_bin_path,
        "list",
        "--json",
        "title",
        "--json",
        "start-date",
        "--json",
        "start-time",
        "--json",
        "end-date",
        "--json",
        "end-time",
        "--json",
        "all-day",
        arguments.from,
        &format!("{}d", arguments.duration_days),
    ];

    let data: AS::Data = agenda_source.obtain(&args).map_err(Error::Io)?;
    let bytes: &str = std::str::from_utf8(data.as_ref()).map_err(Error::InvalidUnicode)?;
    let mut agenda = Agenda::new();
    for part in bytes.split('\n').filter(|p| !p.is_empty()) {
        let agenda_part = json_parser.parse(part).map_err(Error::Parse)?;
        agenda.extend(agenda_part);
    }
    Ok(agenda)
}
