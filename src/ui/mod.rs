mod game_over;
mod menu;
mod name_entry;
mod play;

use ratatui::Frame;

use crate::app::{App, Screen};

pub fn draw(frame: &mut Frame, app: &App) {
    match app.screen {
        Screen::Menu => menu::draw(frame, app),
        Screen::Playing | Screen::Dying { .. } => play::draw(frame, app),
        Screen::NameEntry { .. } => name_entry::draw(frame, app),
        Screen::GameOver { .. } => game_over::draw(frame, app),
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;
    use crate::{
        db::{Db, ScoreRow},
        game::Mode,
    };

    fn render(app: &App) -> String {
        let mut t = Terminal::new(TestBackend::new(60, 24)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        t.backend()
            .buffer()
            .content()
            .chunks(60)
            .map(|row| row.iter().map(|c| c.symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app() -> App {
        App::new(Db::open_in_memory().unwrap()).unwrap()
    }

    #[test]
    fn menu_empty_shows_placeholder_and_keys() {
        let s = render(&app());
        for want in [
            " snake ",
            "no scores yet",
            "1-9  fixed speed",
            "v    variable speed",
            "q    quit",
            "apple = current level in points",
        ] {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    #[test]
    fn menu_lists_top_rows() {
        let mut a = app();
        a.top = vec![
            ScoreRow { name: "ABC".into(), score: 42, mode: Mode::Fixed(5) },
            ScoreRow { name: "XYZ".into(), score: 7, mode: Mode::Variable },
        ];
        let s = render(&a);
        assert!(s.contains(" 1. ABC     42  fixed 5"), "{s}");
        assert!(s.contains(" 2. XYZ      7  variable"), "{s}");
        assert!(!s.contains("no scores yet"));
    }

    #[test]
    fn play_title_shows_status() {
        let mut a = app();
        a.game = crate::game::Game::new(Mode::Fixed(3));
        a.game.apples = 2;
        a.game.score = 6;
        a.screen = Screen::Playing;
        let s = render(&a);
        assert!(s.contains(" fixed 3  level 3  apples 2  score 6  q quit "), "{s}");
    }

    #[test]
    fn name_entry_pads_name() {
        let mut a = app();
        a.screen = Screen::NameEntry { game_id: 1, name: "A".into() };
        let s = render(&a);
        for want in [" high score ", "enter your name", "A__", "Enter to confirm"] {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }

    #[test]
    fn game_over_shows_result_and_keys() {
        let mut a = app();
        a.game.score = 12;
        a.game.apples = 4;
        a.screen = Screen::GameOver { ranked: false };
        let s = render(&a);
        for want in [" game over ", "score 12", "apples 4", "variable", "r  menu", "q  quit"] {
            assert!(s.contains(want), "missing {want:?} in\n{s}");
        }
    }
}
