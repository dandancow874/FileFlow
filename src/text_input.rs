use std::ops::Range;

use gpui::prelude::*;
use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, Element, ElementId, ElementInputHandler,
    Entity, EntityInputHandler, FocusHandle, Focusable, GlobalElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Render,
    ShapedLine, SharedString, Style, TextRun, UTF16Selection, Window, actions, div, fill, point,
    px, rgba, size,
};
use unicode_segmentation::UnicodeSegmentation;

actions!(
    fileflow_text,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        InsertSpace,
        InsertNewline,
        Up,
        Down
    ]
);

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Vec<(usize, ShapedLine)>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    multiline: bool,
    scroll_handle: gpui::ScrollHandle,
    reveal_cursor: bool,
    line_height: Pixels,
}

impl TextInput {
    pub fn new(cx: &mut Context<Self>, placeholder: impl Into<SharedString>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: "".into(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: Vec::new(),
            last_bounds: None,
            is_selecting: false,
            multiline: false,
            scroll_handle: gpui::ScrollHandle::new(),
            reveal_cursor: false,
            line_height: px(24.),
        }
    }
    pub fn value(&self) -> String {
        self.content.to_string()
    }
    pub fn set_multiline(&mut self, multiline: bool) {
        self.multiline = multiline;
    }
    pub fn insert_newline(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.multiline {
            self.replace_text_in_range(None, "\n", window, cx);
        }
    }
    pub fn set_value(&mut self, value: impl Into<SharedString>) {
        self.content = value.into();
        self.select_all_internal();
        self.scroll_handle.set_offset(point(px(0.), px(0.)));
    }
    /// 设置内容并只选中开头到 `select_until` 的部分（UTF-8 字节偏移）。
    /// 重命名时传主文件名长度：xxx.jpg 只选中 "xxx"，后缀不被覆盖。
    pub fn set_value_select_until(&mut self, value: impl Into<SharedString>, select_until: usize) {
        self.content = value.into();
        let end = select_until.min(self.content.len());
        self.selected_range = 0..end;
        self.selection_reversed = false;
        self.marked_range = None;
    }
    pub fn clear(&mut self) {
        self.content = "".into();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
    }
    pub fn select_all_internal(&mut self) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
    }
    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }
    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.reveal_cursor = true;
        cx.notify();
    }
    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.reveal_cursor = true;
        cx.notify();
    }
    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(i, _)| (i < offset).then_some(i))
            .unwrap_or(0)
    }
    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(i, _)| (i > offset).then_some(i))
            .unwrap_or(self.content.len())
    }
    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.content.chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }
    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.content.chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }
    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range.start)..self.offset_from_utf16(range.end)
    }
    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }
    fn index_for_mouse_position(&self, position: gpui::Point<Pixels>) -> usize {
        // Placeholder text is visual only. Its glyph offsets must never become a
        // selection range for the empty underlying value.
        if self.content.is_empty() {
            return 0;
        }
        let Some(bounds) = &self.last_bounds else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        let row = ((position.y - bounds.top()) / self.line_height).floor().max(0.) as usize;
        let Some((offset, line)) = self.last_layout.get(row.min(self.last_layout.len().saturating_sub(1))) else {
            return 0;
        };
        (offset + line.closest_index_for_x(position.x - bounds.left())).min(self.content.len())
    }
    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        let index = if self.selected_range.is_empty() {
            self.previous_boundary(self.cursor_offset())
        } else {
            self.selected_range.start
        };
        self.move_to(index, cx);
    }
    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        let index = if self.selected_range.is_empty() {
            self.next_boundary(self.cursor_offset())
        } else {
            self.selected_range.end
        };
        self.move_to(index, cx);
    }
    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }
    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }
    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all_internal();
        cx.notify();
    }
    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let offset = if self.multiline { self.content[..self.cursor_offset()].rfind('\n').map_or(0, |ix| ix + 1) } else { 0 };
        self.move_to(offset, cx);
    }
    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let end = if self.multiline { self.content[offset..].find('\n').map_or(self.content.len(), |ix| offset + ix) } else { self.content.len() };
        self.move_to(end, cx);
    }
    fn move_vertical(&mut self, direction: isize, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let row = self.last_layout.iter().rposition(|(start, _)| *start <= offset).unwrap_or(0);
        let target = (row as isize + direction).clamp(0, self.last_layout.len().saturating_sub(1) as isize) as usize;
        if let (Some((start, line)), Some((target_start, target_line))) = (self.last_layout.get(row), self.last_layout.get(target)) {
            let x = line.x_for_index(offset - start);
            self.move_to(target_start + target_line.closest_index_for_x(x), cx);
        }
    }
    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) { self.move_vertical(-1, cx); }
    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) { self.move_vertical(1, cx); }
    fn newline(&mut self, _: &InsertNewline, window: &mut Window, cx: &mut Context<Self>) { self.insert_newline(window, cx); }
    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.previous_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.select_to(self.next_boundary(self.cursor_offset()), cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }
    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }
    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        self.copy(&Copy, window, cx);
        if !self.selected_range.is_empty() {
            self.replace_text_in_range(None, "", window, cx);
        }
    }
    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            let text = if self.multiline { text.replace("\r\n", "\n").replace('\r', "\n") } else { text.replace(['\n', '\r'], "") };
            self.replace_text_in_range(None, &text, window, cx);
        }
    }
    fn insert_space(&mut self, _: &InsertSpace, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, " ", window, cx);
    }
    fn on_mouse_down(&mut self, event: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx);
        }
    }
    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }
    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }
    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked_range = None;
    }
    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.selected_range = range.start + new_text.len()..range.start + new_text.len();
        self.marked_range.take();
        self.reveal_cursor = true;
        cx.notify();
    }
    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[..range.start].to_owned() + new_text + &self.content[range.end..]).into();
        self.marked_range =
            (!new_text.is_empty()).then_some(range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .map(|range| range.start + range.start..range.end + range.end)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        self.reveal_cursor = true;
        cx.notify();
    }
    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        let row = self.last_layout.iter().rposition(|(start, _)| *start <= range.start)?;
        let (start, line) = &self.last_layout[row];
        let top = bounds.top() + self.line_height * row as f32;
        Some(Bounds::from_corners(
            point(bounds.left() + line.x_for_index(range.start - start), top),
            point(bounds.left() + line.x_for_index((range.end - start).min(line.len())), top + self.line_height),
        ))
    }
    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        self.last_bounds?.localize(&point)?;
        Some(self.offset_to_utf16(self.index_for_mouse_position(point)))
    }
}

pub struct TextElement {
    pub input: Entity<TextInput>,
}
pub struct PrepaintState {
    lines: Vec<(usize, ShapedLine)>,
    cursor: Option<PaintQuad>,
    selections: Vec<PaintQuad>,
}
impl IntoElement for TextElement {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = gpui::relative(1.).into();
        let input = self.input.read(cx);
        let rows = if input.multiline { input.content.split('\n').count() } else { 1 };
        style.size.height = (window.line_height() * rows as f32).into();
        style.flex_shrink = 0.;
        (window.request_layout(style, [], cx), ())
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let selected = input.selected_range.clone();
        let cursor_offset = input.cursor_offset();
        let style = window.text_style();
        let display = if input.content.is_empty() { input.placeholder.clone() } else { input.content.clone() };
        let line_height = window.line_height();
        let mut lines = Vec::new();
        let mut selections = Vec::new();
        let mut cursor = None;
        let mut offset = 0;
        for (row, text) in display.split('\n').enumerate() {
            let top = bounds.top() + line_height * row as f32;
            let local_start = selected.start.saturating_sub(offset).min(text.len());
            let local_end = selected.end.saturating_sub(offset).min(text.len());
            let mut runs = Vec::new();
            for (start, end, selected_text) in [(0, local_start, false), (local_start, local_end, true), (local_end, text.len(), false)] {
                if start < end {
                    runs.push(TextRun {
                        len: end - start,
                        font: style.font(),
                        color: if input.content.is_empty() { rgba(0x8a929966).into() } else if selected_text { rgba(0xffffffff).into() } else { style.color },
                        background_color: None,
                        underline: None,
                        strikethrough: None,
                    });
                }
            }
            let line = window.text_system().shape_line(text.to_owned().into(), style.font_size.to_pixels(window.rem_size()), &runs, None);
            if selected.is_empty() && cursor_offset >= offset && cursor_offset <= offset + text.len() {
                cursor = Some(fill(Bounds::new(point(bounds.left() + line.x_for_index(cursor_offset - offset), top), size(px(1.), line_height)), rgba(0x0078d4ff)));
            } else if !selected.is_empty() && selected.start <= offset + text.len() && selected.end > offset {
                let start_x = line.x_for_index(local_start);
                let end_x = if selected.end > offset + text.len() { line.width + px(4.) } else { line.x_for_index(local_end) };
                selections.push(fill(Bounds::from_corners(point(bounds.left() + start_x, top), point(bounds.left() + end_x, top + line_height)), rgba(0x0078d4ff)));
            }
            lines.push((offset, line));
            offset += text.len() + 1;
        }
        PrepaintState { lines, cursor, selections }
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        state: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        for selection in state.selections.drain(..) { window.paint_quad(selection); }
        let lines = std::mem::take(&mut state.lines);
        for (row, (_, line)) in lines.iter().enumerate() {
            line.paint(point(bounds.left(), bounds.top() + window.line_height() * row as f32), window.line_height(), window, cx).unwrap();
        }
        if handle.is_focused(window)
            && let Some(cursor) = state.cursor.take()
        {
            window.paint_quad(cursor);
        }
        self.input.update(cx, |input, _| {
            input.last_layout = lines;
            input.last_bounds = Some(bounds);
            input.line_height = window.line_height();
        });
    }
}
impl Render for TextInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.multiline && self.reveal_cursor {
            let row = self.content[..self.cursor_offset()].matches('\n').count();
            let top = window.line_height() * row as f32;
            let viewport = px(144.);
            let mut offset = self.scroll_handle.offset();
            if top + window.line_height() > -offset.y + viewport { offset.y = -(top + window.line_height() - viewport); }
            if top < -offset.y { offset.y = -top; }
            self.scroll_handle.set_offset(offset);
            self.reveal_cursor = false;
        }
        div()
            .id("text-input")
            .flex()
            .flex_col()
            .key_context(if self.multiline { "FileFlowTextInput FileFlowMultilineInput" } else { "FileFlowTextInput" })
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::insert_space))
            .when(self.multiline, |input| input
                .overflow_y_scroll()
                .track_scroll(&self.scroll_handle)
                .on_action(cx.listener(Self::newline))
                .on_action(cx.listener(Self::up))
                .on_action(cx.listener(Self::down)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .w_full()
            .h_full()
            .child(TextElement { input: cx.entity() })
    }
}
impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
