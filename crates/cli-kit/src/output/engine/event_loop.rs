use super::Event;

pub fn read_event() -> Result<Event, std::io::Error> {
    let event = crossterm::event::read()?;
    Ok(Event::from(event))
}

pub fn poll_event(timeout: std::time::Duration) -> Result<Option<Event>, std::io::Error> {
    if crossterm::event::poll(timeout)? {
        let event = crossterm::event::read()?;
        Ok(Some(Event::from(event)))
    } else {
        Ok(None)
    }
}

pub fn enter_raw_mode() -> Result<(), std::io::Error> {
    crossterm::terminal::enable_raw_mode()
}

pub fn leave_raw_mode() -> Result<(), std::io::Error> {
    crossterm::terminal::disable_raw_mode()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_poll_timeout() {
        // Polling with zero timeout may fail if not in a TTY; just confirm no panic.
        let _ = poll_event(std::time::Duration::from_millis(0));
    }

    #[test]
    fn test_event_conversion() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let input =
            crossterm::event::Event::Key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        let event = Event::from(input);
        match event {
            Event::Key(key) => assert_eq!(key.code, KeyCode::Char('q')),
            _ => panic!("expected Key event"),
        }
    }
}
