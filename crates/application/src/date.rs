use super::{TimeError, sdl};
use calendar::date::Date;

pub fn get_week_start(now: sdl::SDL_Time) -> Result<Date, TimeError> {
    let local_time = true;
    unsafe {
        let today = sdlext::time_to_date_time(now, local_time)?;
        //let sunday: std::ffi::c_int = 0;
        // from 0 to 6
        let current_weekday: std::ffi::c_int =
            sdl::SDL_GetDayOfWeek(today.year, today.month, today.day);
        //let mut week_offset: std::ffi::c_int = current_weekday - sunday;

        let natural_weekday: i32 = match current_weekday {
            0 => 7,
            d @ 1..7 => d,
            _ => panic!("SDL has returned an invalid week day"),
        };
        // from 1 to 7
        //let natural_weekday: i32 = current_weekday + 7 * ((7 - current_weekday) / 7);
        // TODO: implement for the case Sunday is the first day.
        // monday => 0
        // ...
        // sunday => -6
        let week_offset_days: i32 = -(natural_weekday - 1);
        const NANOSECONDS_PER_DAY: i64 = (sdl::SDL_NS_PER_SECOND as i64) * 60 * 60 * 24;
        let offset_ns: i64 = (week_offset_days as i64) * NANOSECONDS_PER_DAY;
        let mut first_day_of_week: sdl::SDL_DateTime = std::mem::zeroed();
        if !sdl::SDL_TimeToDateTime(
            now + offset_ns,
            &mut first_day_of_week as *mut _,
            local_time,
        ) {
            return Err(TimeError::FailConvertingNowToDate);
        }

        Ok(Date {
            year: first_day_of_week.year as u16,
            month: first_day_of_week.month as u8,
            day: first_day_of_week.day as u8,
        })
    }
}
