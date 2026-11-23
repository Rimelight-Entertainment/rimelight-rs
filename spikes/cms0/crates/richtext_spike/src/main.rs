//! Input-sink overlay spike: a transparent TextInput owns input; a read-only
//! StyledText underneath shows the rich text generated from `model::Para`.
mod model;

use model::{Mark, Para};
use slint::ComponentHandle;
use std::{cell::RefCell, ops::Range, rc::Rc};

slint::include_modules!();

fn seed() -> Para {
    let pieces = vec![
        ("Olá, tudo bem? Ação e coração: ", None),
        ("negrito", Some(Mark::Bold)),
        (" ", None),
        ("itálico", Some(Mark::Italic)),
        (" ", None),
        ("sublinhado", Some(Mark::Underline)),
        (" ", None),
        ("riscado", Some(Mark::Strike)),
        (" ", None),
        ("código", Some(Mark::Code)),
        (" 😀 ", None),
        ("@Ana", Some(Mark::Mention("entry-123".into()))),
        (" fim.", None),
    ];
    let mut text = String::new();
    let mut marks = Vec::new();
    for (s, mark) in pieces {
        let start = text.len();
        text.push_str(s);
        if let Some(m) = mark {
            marks.push((start..text.len(), m));
        }
    }
    Para { text, marks }
}

fn mark_for(name: &str) -> Option<Mark> {
    Some(match name {
        "bold" => Mark::Bold,
        "italic" => Mark::Italic,
        "underline" => Mark::Underline,
        "strike" => Mark::Strike,
        "code" => Mark::Code,
        _ => return None,
    })
}

fn selection(ui: &MainWindow) -> Range<usize> {
    let (a, c) = (ui.get_anchor() as usize, ui.get_caret() as usize);
    a.min(c)..a.max(c)
}

fn render(ui: &MainWindow, para: &Para) {
    let md = model::to_markdown(para);
    // Assumed API: from_markdown returns StyledText directly (see README / report).
    ui.set_styled(slint::StyledText::from_markdown(&md).unwrap_or_else(|e| { eprintln!("markdown error: {e}
{md}"); slint::StyledText::from_plain_text(&md) }));
    ui.set_markdown_debug(md.into());
}

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;
    let para = Rc::new(RefCell::new(seed()));
    ui.set_plain(para.borrow().text.as_str().into());
    render(&ui, &para.borrow());

    let weak = ui.as_weak();

    ui.on_text_edited({
        let (weak, para) = (weak.clone(), para.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let new = ui.get_plain();
            let mut p = para.borrow_mut();
            p.edit(new.as_str());
            render(&ui, &p);
        }
    });

    ui.on_toggle_mark({
        let (weak, para) = (weak.clone(), para.clone());
        move |name| {
            let (Some(ui), Some(mark)) = (weak.upgrade(), mark_for(name.as_str())) else {
                return;
            };
            let range = selection(&ui);
            let mut p = para.borrow_mut();
            p.toggle(range, &mark);
            render(&ui, &p);
        }
    });

    ui.on_insert_mention({
        let (weak, para) = (weak.clone(), para.clone());
        move || {
            let Some(ui) = weak.upgrade() else { return };
            let at = ui.get_caret() as usize;
            let mut p = para.borrow_mut();
            let end = p.insert(at, "@Ana", Some(Mark::Mention("entry-123".into())));
            ui.set_plain(p.text.as_str().into());
            ui.invoke_set_selection(end as i32, end as i32);
            ui.invoke_focus_input();
            render(&ui, &p);
        }
    });

    ui.run()
}
