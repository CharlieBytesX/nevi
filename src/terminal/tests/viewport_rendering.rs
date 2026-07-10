use super::render_editor_to_string;
use crate::editor::Editor;
use crate::input::Motion;

#[test]
fn full_render_after_wrapped_file_end_shows_context_above_last_line() {
    let mut editor = Editor::default();
    editor.set_size(80, 24);
    editor.settings.editor.scroll_off = 8;
    editor.settings.editor.wrap = true;
    editor.settings.editor.wrap_width = 80;
    let content = (1..=100)
        .map(|line| format!("line {line:03}\n"))
        .collect::<String>();
    editor.replace_buffer_content(&content);
    editor.apply_motion(Motion::FileEnd, 1);

    let rendered = render_editor_to_string(&editor);

    assert!(
        rendered.contains("line 079"),
        "render should keep visible context above the final line; output={rendered:?}"
    );
    assert!(
        rendered.contains("line 100"),
        "render should still include the final line; output={rendered:?}"
    );
}

#[test]
fn full_render_after_wrapped_file_end_shows_tall_final_line_cursor_segment() {
    let mut editor = Editor::default();
    editor.set_size(80, 24);
    editor.settings.editor.scroll_off = 8;
    editor.settings.editor.wrap = true;
    editor.settings.editor.wrap_width = 20;
    editor.replace_buffer_content(&format!("context\nCURSOR_PREFIX{}\n", "x".repeat(5_000)));
    editor.apply_motion(Motion::FileEnd, 1);

    let rendered = render_editor_to_string(&editor);

    assert!(
        rendered.contains("CURSOR_PREFIX"),
        "render should include the final line cursor segment; output={rendered:?}"
    );
}

#[test]
fn full_render_after_wrapped_file_end_in_horizontal_split_shows_final_context() {
    let mut editor = Editor::default();
    editor.set_size(80, 24);
    editor.settings.editor.scroll_off = 8;
    editor.settings.editor.wrap = true;
    editor.settings.editor.wrap_width = 80;
    let content = (1..=100)
        .map(|line| format!("line {line:03}\n"))
        .collect::<String>();
    editor.replace_buffer_content(&content);
    editor.hsplit(None).expect("horizontal split");
    editor.apply_motion(Motion::FileEnd, 1);

    let rendered = render_editor_to_string(&editor);

    assert!(
        rendered.contains("line 090"),
        "active split should show context above EOF; output={rendered:?}"
    );
    assert!(
        rendered.contains("line 100"),
        "active split should show the final cursor line; output={rendered:?}"
    );
}
