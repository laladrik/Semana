use nanoserde::DeJson;
use nanoserde::SerJson;
// extern crate alloc;
//  use alloc::string::String;
//  use alloc::vec::Vec;
//  use alloc::str;

use std::ffi::OsStr;
use std::io::Write;
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

pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl DeJson for Date {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut core::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        let year: u16 = parse_digits::<4, u16>(state, input)?;
        let month: u8 = {
            let two_digit_number = parse_two_digits(state, input)?;
            if two_digit_number > 12 {
                return Err(state.err_parse("invalid month"));
            }
            two_digit_number
        };

        skip_delimeter(state, input, ':')?;
        let day: u8 = {
            let two_digit_number = parse_two_digits(state, input)?;
            if two_digit_number > 31 {
                return Err(state.err_parse("invalid day"));
            }
            two_digit_number
        };

        Ok(Date { year, month, day })
    }
}

pub struct Time {
    pub hour: u8,
    pub minute: u8,
}

fn parse_digits<const N: usize, Out: core::str::FromStr>(
    state: &mut nanoserde::DeJsonState,
    input: &mut core::str::Chars,
) -> Result<Out, nanoserde::DeJsonErr> {
    let mut ret: [char; N] = [' '; N];
    for i in 0..N {
        state.next(input);
        let maybe_digit = state.cur;
        ret[i] = maybe_digit;
    }

    let s = String::from_iter(ret.iter());
    Out::from_str(&s).map_err(|_| state.err_parse("the hour of the time is invalid"))
}

fn parse_two_digits(
    state: &mut nanoserde::DeJsonState,
    input: &mut core::str::Chars,
) -> Result<u8, nanoserde::DeJsonErr> {
    parse_digits::<2, u8>(state, input)
}

fn skip_delimeter(
    state: &mut nanoserde::DeJsonState,
    input: &mut core::str::Chars,
    expected_delimeter: char,
) -> Result<(), nanoserde::DeJsonErr> {
    state.next(input);
    let actual_delimeter = state.cur;
    if actual_delimeter != expected_delimeter {
        Err(nanoserde::DeJsonErr {
            msg: nanoserde::DeJsonErrReason::CannotParse("colon after the hour in Time".to_owned()),
            line: state.line,
            col: state.col,
        })
    } else {
        Ok(())
    }
}

// parses  a string like 12:34, 09:23
impl DeJson for Time {
    fn de_json(
        state: &mut nanoserde::DeJsonState,
        input: &mut core::str::Chars,
    ) -> Result<Self, nanoserde::DeJsonErr> {
        let hour: u8 = {
            let two_digit_number = parse_two_digits(state, input)?;
            if two_digit_number > 23 {
                return Err(state.err_parse("the hour is too big"));
            }
            two_digit_number
        };

        skip_delimeter(state, input, ':')?;
        let minute: u8 = {
            let two_digit_number = parse_two_digits(state, input)?;
            if two_digit_number > 59 {
                return Err(state.err_parse("the minute is too big"));
            }
            two_digit_number
        };

        Ok(Time { hour, minute })
    }
}

#[derive(DeJson, Debug)]
pub struct Item {
    pub title: String,
    #[nserde(rename = "start-date")]
    pub start_date: String,
    #[nserde(rename = "start-time")]
    pub start_time: String,
    #[nserde(rename = "end-date")]
    pub end_date: String,
    #[nserde(rename = "end-time")]
    pub end_time: String,
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
        ObtainArguments { from, duration_days: 7, backend_bin_path: "khal" }
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
        return Err(Error::DurationIsTooBig)
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

pub fn parse() {
    let input = r#"[{"title": "MC Kieran office hours"}, {"title": "Café"}, {"title": "Ejercicio r2"}, {"title": "Desayuno"}, {"title": "Almuerzo"}, {"title": "Tocar la batería"}, {"title": "Networking"}, {"title": "Preparar para dormir"}]"#;
    let _: Agenda = DeJson::deserialize_json(input).unwrap();
}

#[test]
fn rename() {
    #[derive(DeJson, SerJson, PartialEq)]
    #[nserde(default)]
    pub struct Test {
        #[nserde(rename = "foo-field")]
        pub a: i32,
        #[nserde(rename = "bar-field")]
        pub b: Bar,
    }

    #[derive(DeJson, SerJson, PartialEq, Debug)]
    pub enum Bar {
        #[nserde(rename = "fooValue")]
        A,
        #[nserde(rename = "barValue")]
        B,
    }

    impl Default for Bar {
        fn default() -> Self {
            Self::A
        }
    }

    let json = r#"{
        "foo-field": 1,
        "bar-field": "fooValue",
    }"#;

    let test: Test = DeJson::deserialize_json(json).unwrap();
    assert_eq!(test.a, 1);
    assert_eq!(test.b, Bar::A);

    let bytes = SerJson::serialize_json(&test);
    let test_deserialized = DeJson::deserialize_json(&bytes).unwrap();
    assert!(test == test_deserialized);
}
