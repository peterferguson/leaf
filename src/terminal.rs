//! Terminal commands that accept untrusted document text.

use crossterm::terminal::SetTitle;

pub fn set_title(text: &str) -> SetTitle<String> {
    // Both C0 (ESC, BEL) and C1 (ST, CSI) controls can break out of OSC.
    SetTitle(text.chars().filter(|c| !c.is_control()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::Command;

    #[test]
    fn document_title_cannot_end_osc_or_inject_clipboard_commands() {
        let title = "Paper\x07\x1b]52;c;YXR0YWNr\x07\x1b\\尾\u{009c}\u{009b}31m";
        let mut output = String::new();
        set_title(title).write_ansi(&mut output).unwrap();
        assert_eq!(output, "\x1b]0;Paper]52;c;YXR0YWNr\\尾31m\x07");
        assert_eq!(output.matches('\x1b').count(), 1);
        assert_eq!(output.matches('\x07').count(), 1);
    }

    #[test]
    fn ordinary_unicode_titles_are_preserved() {
        let mut output = String::new();
        set_title("Résumé 日本語 📖")
            .write_ansi(&mut output)
            .unwrap();
        assert_eq!(output, "\x1b]0;Résumé 日本語 📖\x07");
    }
}
