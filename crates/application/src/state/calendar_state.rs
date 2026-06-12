static NO_RECT: calendar::render::Rectangles = Vec::new();
static NO_TITLES: Vec<String> = Vec::new();

// FIXME: Flatten the structure
pub struct WeekData {
    pub agenda: calendar::obtain::WeekScheduleWithLanes,
}

pub enum CalendarState<Handle> {
    Loading {
        agenda_source_handle: Handle,
    },
    Ready {
        week_data: WeekData,
        long_event_clash_size: calendar::Lane,
        short_event_rectangles_opt: calendar::render::Rectangles,
        long_event_rectangles_opt: calendar::render::Rectangles,
    },
    Rendering {
        week_data: WeekData,
        long_event_clash_size: calendar::Lane,
    },
}

impl<Handle> CalendarState<Handle> {
    pub fn obtain_events<'a>(&'a self) -> EventRectangles<'a> {
        match self {
            Self::Loading { .. } => EventRectangles {
                long: &NO_RECT,
                short: &NO_RECT,
            },
            Self::Ready {
                short_event_rectangles_opt,
                long_event_rectangles_opt,
                ..
            } => EventRectangles {
                long: long_event_rectangles_opt,
                short: short_event_rectangles_opt,
            },
            Self::Rendering { .. } => {
                unreachable!("unexpected state of the calendar")
            }
        }
    }

    pub fn get_event_table(&self, is_event_long: bool) -> Option<&calendar::EventTable> {
        if let Self::Ready { week_data, .. } = self {
            Some(match is_event_long {
                true => &week_data.agenda.long,
                false => &week_data.agenda.short,
            })
        } else {
            None
        }
    }

    pub fn obtain_long_events_titles(&self) -> &[impl AsRef<str>] {
        if let Self::Ready { week_data, .. } = self {
            week_data.agenda.long.titles.as_slice()
        } else {
            NO_TITLES.as_slice()
        }
    }

    pub fn obtain_short_events_titles(&self) -> &[impl AsRef<str>] {
        if let Self::Ready { week_data, .. } = self {
            week_data.agenda.short.titles.as_slice()
        } else {
            NO_TITLES.as_slice()
        }
    }

    /// It provides a memory-safe way to switch the state.  The function creates an uninitialized
    /// state to replace the current one.  Then it tries to switch to the next state provided by
    /// the function `update`.  The function must return any valid state and an error if any has
    /// occurred.
    pub fn switch<E>(&mut self, update: impl FnOnce(Self) -> (Self, Option<E>)) -> Result<(), E> {
        // SAFETY: the bald_state is never read until the function finishes.
        let bald_state: CalendarState<_> = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        let current_state = std::mem::replace(self, bald_state);
        let (new_state, error) = update(current_state);
        *self = new_state;
        match error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    pub fn switch_infallible(&mut self, update: impl Fn(Self) -> Self) {
        // SAFETY: the bald_state is never read until the function finishes.
        // FIXME(alex): the state is 200 bytes long.
        let bald_state: CalendarState<_> = unsafe { std::mem::MaybeUninit::zeroed().assume_init() };
        let current_state = std::mem::replace(self, bald_state);
        let new_state = update(current_state);
        *self = new_state;
    }

    /// a shortcut to switch to the [`CalendarState<H>::Loading`]
    pub fn loading<E>(agenda_source_handle: Handle, e: Option<E>) -> (Self, Option<E>) {
        (
            Self::Loading {
                agenda_source_handle,
            },
            e,
        )
    }

    pub fn get_long_event_clash_size(&self) -> calendar::Lane {
        match self {
            CalendarState::Loading { .. } => 0,
            CalendarState::Ready {
                long_event_clash_size,
                ..
            }
            | CalendarState::Rendering {
                long_event_clash_size,
                ..
            } => *long_event_clash_size,
        }
    }
}

pub struct EventRectangles<'rect> {
    pub long: &'rect calendar::render::Rectangles,
    pub short: &'rect calendar::render::Rectangles,
}
