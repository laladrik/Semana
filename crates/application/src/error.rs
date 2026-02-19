use sdlext::TimeError;

type JsonParseError = <calendar::obtain::NanoSerde as calendar::obtain::JsonParser>::Error;
type AgendaObtainError = calendar::obtain::Error<JsonParseError>;

#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    Sdl(sdlext::Error),
    Calendar(CalendarError),
    DataIsNotAvailable(AgendaObtainError),
}

impl From<FrontendError> for Error {
    fn from(value: FrontendError) -> Self {
        match value {
            FrontendError::WeekStartIsNotObtained(e) => Error::from(sdlext::Error::from(e)),
            FrontendError::CStringIsNotCreated(_nul_error) => todo!("handle zeroes in UTF-8"),
            FrontendError::TextObjectIsNotRegistered(e) => Error::from(e),
            FrontendError::AgendaSourceFailed(e) => Error::from(e),
        }
    }
}

impl From<sdlext::Error> for Error {
    fn from(value: sdlext::Error) -> Self {
        Error::Sdl(value)
    }
}

impl From<CalendarError> for Error {
    fn from(value: CalendarError) -> Self {
        Error::Calendar(value)
    }
}

impl From<AgendaObtainError> for Error {
    fn from(value: AgendaObtainError) -> Self {
        Error::DataIsNotAvailable(value)
    }
}

pub enum FrontendError {
    // FIXME(alex): figure out how to store the text of the SDL errors.
    AgendaSourceFailed(sdlext::Error),
    WeekStartIsNotObtained(TimeError),
    CStringIsNotCreated(std::ffi::NulError),
    TextObjectIsNotRegistered(sdlext::Error),
}

#[derive(Debug)]
pub struct CalendarError {
    _data: String,
}

impl<'event> From<calendar::Error<'event>> for CalendarError {
    fn from(value: calendar::Error<'event>) -> Self {
        let (calendar::Error::InvalidDate(data) | calendar::Error::InvalidTime(data)) = value;
        Self {
            _data: data.to_owned(),
        }
    }
}
