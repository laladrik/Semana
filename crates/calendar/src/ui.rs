use super::TextCreate;

pub struct Week<Text> {
    pub days: [Text; 7],
    pub hours: [Text; 24],
    pub dates: [Text; 7],
}

/// create a structure with all of the texts for the week view.
///
/// # Panics
///
/// if `date_stream` does not provide 7 elements.
pub fn create_texts<TF, R, I, D>(text_factory: &TF, date_stream: I) -> Week<R>
where
    TF: TextCreate<Result = R>,
    I: Iterator<Item = D>,
    D: std::borrow::Borrow<super::Date>,
{
    let mut dates_iter = create_date_texts(text_factory, date_stream);
    let dates: [R; 7] = core::array::from_fn(|_| {
        dates_iter
            .next()
            .expect("date_stream didn't sufficient amount of elements")
    });

    Week {
        days: create_weekday_texts(text_factory),
        hours: create_hours_texts(text_factory),
        dates,
    }
}

pub fn create_hours_texts<TF, R>(text_factory: &TF) -> [R; 24]
where
    TF: TextCreate<Result = R>,
{
    let hours: [R; 24] = core::array::from_fn(|i| {
        let s = format!("{:02}:00", i);
        text_factory.text_create(s.as_str())
    });
    hours
}

pub fn create_weekday_texts<TF, R>(text_factory: &TF) -> [R; 7]
where
    TF: TextCreate<Result = R>,
{
    let weekdays = [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ];
    let ret: [R; 7] = core::array::from_fn(|i| text_factory.text_create(weekdays[i]));
    ret
}

pub fn create_date_texts<TF, R, I, D>(text_factory: &TF, dates: I) -> impl Iterator<Item = R>
where
    TF: TextCreate<Result = R>,
    I: Iterator<Item = D>,
    D: std::borrow::Borrow<super::Date>,
{
    dates.map(|date| {
        let date: &super::Date = date.borrow();
        let text = format!("{:04}-{:02}-{:02}", date.year, date.month, date.day);
        text_factory.text_create(&text)
    })
}

pub fn create_event_title_texts<'text, 'tf, TF, R, I>(
    text_factory: &'tf TF,
    items: I,
) -> impl Iterator<Item = R>
where
    TF: TextCreate<Result = R> + 'tf,
    I: Iterator<Item = &'text str>,
{
    items.map(|text| text_factory.text_create(text))
}
