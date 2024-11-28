use crossterm::event::{self, Event};
use ratatui::widgets::{Block, List, ListDirection, ListState};
use ratatui::prelude::*;
use ratatui::{prelude::CrosstermBackend, text::Text, Frame, Terminal};
use ratatui::layout::Constraint::{Fill, Length, Min};


use crate::state::{self as state, Dispatcher};

pub struct AppDisplay {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    state: state::UiState,
    frame_count: usize,
    query_list: ListState,
    dispatcher_tx: tokio::sync::mpsc::UnboundedSender<state::Action>,
}

impl AppDisplay {
    pub fn new(state: state::UiState, dispatcher_tx: tokio::sync::mpsc::UnboundedSender<state::Action>) -> Self {
        let mut terminal: ratatui::Terminal<CrosstermBackend<std::io::Stdout>> = ratatui::init();
        terminal.clear();
        let mut query_list_state = ListState::default();
        Self { terminal, state, frame_count: 0, query_list: query_list_state, dispatcher_tx }
    }

    pub fn set_state(&mut self, state: state::UiState) {
        self.state = state;
    }

    pub fn render(&mut self) {
        self.frame_count += 1;
        self.terminal.draw(|frame| {
            let vertical = Layout::vertical([Length(1), Min(0), Length(1)]);
            let [title_area, main_area, status_area] = vertical.areas(frame.area());
            let horizontal = Layout::horizontal([Fill(1), Fill(2)]);
            let [left_area, right_area] = horizontal.areas(main_area);
        
            let mut count = 0;

            // Left area list of query sequence names
            if self.state.query_sequences_list.is_empty() {
                frame.render_widget(Text::styled("No query sequences", Style::default().fg(Color::Red)), left_area);
            } else {
                let mut items = vec![];
                for (i, query) in self.state.query_sequences_list.iter().enumerate() {
                    items.push(query.id.clone());
                }
                count = self.state.query_sequences_list.len();

                if self.query_list.selected().is_none() {
                    self.query_list.select_first();
                    self.dispatcher_tx.send(state::Action::SetSelectedQuery(self.query_list.selected().unwrap())).expect("Unable to send selected query sequence");
                }

                let list = List::new(items)
                    .block(Block::bordered().title("List"))
                    .style(Style::new().white())
                    .highlight_style(Style::new().italic())
                    .highlight_symbol(">>")
                    .repeat_highlight_symbol(true)
                    .direction(ListDirection::TopToBottom);

                frame.render_stateful_widget(list, left_area, &mut self.query_list);
            }

            
        
            frame.render_widget(Block::bordered().title("Fakeminimap2"), title_area);
            frame.render_widget(Block::bordered().title(format!("Status Bar - {} - {}", count, self.frame_count)), status_area);
            frame.render_widget(Block::bordered().title("Query Sequences"), left_area);
            frame.render_widget(Block::bordered().title("Mapping"), right_area);
        
        }).expect("Error rendering");
    }

    pub fn exit(&mut self) {
        ratatui::restore();
    }
}